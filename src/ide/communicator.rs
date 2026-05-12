use serde_json::Value;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const SOCKET_CANDIDATES: &[&str] = &["/tmp/XojoIDE", "/private/tmp/XojoIDE"];
const MAX_RETRIES: u32 = 5;
const RETRY_PAUSE: Duration = Duration::from_millis(1000);

/// Deduplicate socket paths by canonical path.
/// On macOS, /tmp is a symlink to /private/tmp, so both candidates resolve
/// to the same socket. Without deduplication we waste a full timeout cycle
/// on what is effectively a second attempt at the same socket.
fn unique_socket_paths() -> Vec<&'static str> {
    let mut seen = HashSet::new();
    SOCKET_CANDIDATES
        .iter()
        .filter(|p| {
            let canonical = std::fs::canonicalize(p)
                .map(|c| c.to_string_lossy().to_string())
                .unwrap_or_else(|_| p.to_string());
            seen.insert(canonical)
        })
        .copied()
        .collect()
}

/// Classify whether an error is transient and worth retrying.
fn is_retryable(err: &str) -> bool {
    err.contains("not found")
        || err.contains("Connection refused")
        || err.contains("(timeout)")
        || err.contains("connection closed")
}

pub struct Communicator {
    tag_counter: AtomicU64,
    verbose: bool,
}

impl Communicator {
    pub fn new(verbose: bool) -> Self {
        Self {
            tag_counter: AtomicU64::new(0),
            verbose,
        }
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

        let candidates = unique_socket_paths();
        let mut all_errors = Vec::new();
        let mut last_error_retryable = true;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                if !last_error_retryable {
                    break;
                }
                std::thread::sleep(RETRY_PAUSE);
            }

            for path in &candidates {
                if !std::path::Path::new(path).exists() {
                    let err = format!("IPC socket not found at: {path}");
                    all_errors.push(err);
                    // "not found" is retryable — IDE may not have started yet.
                    last_error_retryable = true;
                    continue;
                }

                match self.try_send_receive(path, &payload, &tag, timeout) {
                    Ok(response) => {
                        return Ok(response);
                    }
                    Err(e) => {
                        last_error_retryable = is_retryable(&e);
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
        let mut buffer: Vec<u8> = Vec::with_capacity(8192);
        let mut cursor: usize = 0;
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

            // Consume NUL-delimited frames in place, advancing a cursor rather
            // than reallocating the buffer per frame.
            while let Some(rel_pos) = buffer[cursor..].iter().position(|&b| b == 0) {
                let frame_end = cursor + rel_pos;
                let frame_str = std::str::from_utf8(&buffer[cursor..frame_end])
                    .map_err(|e| format!("Invalid UTF-8 in IPC frame: {e}"))?
                    .trim();
                cursor = frame_end + 1;

                if frame_str.is_empty() {
                    continue;
                }

                if self.verbose {
                    eprintln!("IDE response frame: {frame_str}");
                }

                let response: Value = serde_json::from_str(frame_str)
                    .map_err(|e| format!("Invalid JSON in IPC frame: {e}"))?;

                if response.get("tag").and_then(|t| t.as_str()) == Some(tag) {
                    return Ok(response);
                }
                // Not our tag — discard orphaned frame and keep reading.
            }

            // Periodic compaction: reclaim consumed prefix once it dominates
            // the buffer. Bounded total shift cost — each byte moves at most
            // once between compactions, keeping the loop O(n) amortized.
            if cursor > 4096 && cursor * 2 > buffer.len() {
                buffer.drain(..cursor);
                cursor = 0;
            }
        }
    }

    fn next_tag(&self) -> String {
        let n = self.tag_counter.fetch_add(1, Ordering::Relaxed);
        format!("xmcp_{n}")
    }
}
