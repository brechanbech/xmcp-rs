use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const SOCKET_CANDIDATES: &[&str] = &["/tmp/XojoIDE", "/private/tmp/XojoIDE"];
const MAX_RETRIES: u32 = 5;
const RETRY_PAUSE: Duration = Duration::from_millis(1000);

pub struct Communicator {
    tag_counter: AtomicU64,
    verbose: bool,
    last_error: std::sync::Mutex<String>,
}

impl Communicator {
    pub fn new(verbose: bool) -> Self {
        Self {
            tag_counter: AtomicU64::new(0),
            verbose,
            last_error: std::sync::Mutex::new(String::new()),
        }
    }

    #[allow(dead_code)]
    pub fn last_error_message(&self) -> String {
        self.last_error.lock().unwrap().clone()
    }

    /// Send an IDE script and receive the response. Uses the default 10s timeout.
    #[allow(dead_code)]
    pub fn send_and_receive(&self, script: &str) -> Result<Value, String> {
        self.send_and_receive_with_timeout(script, Duration::from_secs(10))
    }

    /// Send an IDE script and receive the response with a custom timeout.
    pub fn send_and_receive_with_timeout(
        &self,
        script: &str,
        timeout: Duration,
    ) -> Result<Value, String> {
        let tag = self.next_tag();

        // Build protocol v2 payload: handshake + request, NUL-terminated.
        let proto = serde_json::json!({"protocol": 2});
        let request = serde_json::json!({"tag": tag, "script": script});
        let mut payload = Vec::new();
        payload.extend_from_slice(proto.to_string().as_bytes());
        payload.push(0); // NUL terminator
        payload.extend_from_slice(request.to_string().as_bytes());
        payload.push(0); // NUL terminator

        let mut all_errors = Vec::new();
        let mut all_socket_not_found = true;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                // Only retry if all prior errors were "socket not found".
                if !all_socket_not_found {
                    break;
                }
                std::thread::sleep(RETRY_PAUSE);
            }

            for path in SOCKET_CANDIDATES {
                if !std::path::Path::new(path).exists() {
                    let err = format!("IPC socket not found at: {path}");
                    all_errors.push(err);
                    continue;
                }

                match self.try_send_receive(path, &payload, &tag, timeout) {
                    Ok(response) => {
                        *self.last_error.lock().unwrap() = String::new();
                        return Ok(response);
                    }
                    Err(e) => {
                        if !e.contains("not found") {
                            all_socket_not_found = false;
                        }
                        all_errors.push(e);
                    }
                }
            }
        }

        let msg = if all_errors.is_empty() {
            "No IPC socket candidates found.".to_string()
        } else {
            all_errors.join("; ")
        };
        *self.last_error.lock().unwrap() = msg.clone();
        Err(msg)
    }

    fn try_send_receive(
        &self,
        path: &str,
        payload: &[u8],
        tag: &str,
        timeout: Duration,
    ) -> Result<Value, String> {
        let mut stream = UnixStream::connect(path)
            .map_err(|e| format!("IPCSocket connect failed at {path}: {e}"))?;

        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .ok();

        stream
            .write_all(payload)
            .map_err(|e| format!("IPCSocket write failed: {e}"))?;
        stream
            .flush()
            .map_err(|e| format!("IPCSocket flush failed: {e}"))?;

        // Read response frames until we find one with our tag.
        let deadline = Instant::now() + timeout;
        let mut buffer = Vec::new();
        let mut read_buf = [0u8; 4096];

        loop {
            if Instant::now() >= deadline {
                return Err(format!("No IPCSocket response from {path} (timeout)"));
            }

            match stream.read(&mut read_buf) {
                Ok(0) => {
                    return Err(format!("IPCSocket connection closed by {path}"));
                }
                Ok(n) => {
                    buffer.extend_from_slice(&read_buf[..n]);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(format!("IPCSocket read failed: {e}"));
                }
            }

            // Check for NUL-delimited frames.
            while let Some(nul_pos) = buffer.iter().position(|&b| b == 0) {
                let frame_bytes = buffer[..nul_pos].to_vec();
                buffer = buffer[nul_pos + 1..].to_vec();

                let frame_str = std::str::from_utf8(&frame_bytes)
                    .map_err(|e| format!("Invalid UTF-8 in IPC frame: {e}"))?
                    .trim()
                    .to_string();

                if frame_str.is_empty() {
                    continue;
                }

                if self.verbose {
                    eprintln!("IDE response frame: {frame_str}");
                }

                let response: Value = serde_json::from_str(&frame_str)
                    .map_err(|e| format!("Invalid JSON in IPC frame: {e}"))?;

                if response.get("tag").and_then(|t| t.as_str()) == Some(tag) {
                    return Ok(response);
                }
                // Not our tag — discard orphaned frame and keep reading.
            }
        }
    }

    fn next_tag(&self) -> String {
        let n = self.tag_counter.fetch_add(1, Ordering::Relaxed);
        format!("xmcp_{n}")
    }
}
