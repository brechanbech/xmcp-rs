use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::ide::communicator::Communicator;
use crate::mcp::protocol::*;
use crate::mcp::tool::*;
use crate::mcp::resources;

pub struct Server {
    tools: Vec<Box<dyn Tool>>,
    ide: Option<Communicator>,
    docs_path: Option<PathBuf>,
    exe_dir: PathBuf,
    verbose: bool,
}

impl Server {
    pub fn new(
        tools: Vec<Box<dyn Tool>>,
        ide: Option<Communicator>,
        docs_path: Option<PathBuf>,
        exe_dir: PathBuf,
        verbose: bool,
    ) -> Self {
        Self {
            tools,
            ide,
            docs_path,
            exe_dir,
            verbose,
        }
    }

    /// Run the stdin/stdout JSON-RPC loop. Does not return.
    pub fn run(&self) -> ! {
        if self.verbose {
            eprintln!("{SERVER_NAME} starting...");
        }

        let stdin = io::stdin();
        let reader = stdin.lock();
        let stdout = io::stdout();
        let mut writer = stdout.lock();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break, // EOF or IO error
            };
            if line.is_empty() {
                continue;
            }

            if self.verbose {
                eprintln!("{SERVER_NAME} received: {line}");
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    let resp =
                        JsonRpcResponse::error(Value::Null, ErrorCode::ParseError, format!("JSON parsing error: {e}"));
                    Self::write_response(&mut writer, &resp);
                    continue;
                }
            };

            let id = request.id.clone().unwrap_or(Value::Null);

            // Notifications don't have an id.
            if request.id.is_none() {
                let is_notification = request
                    .method
                    .as_deref()
                    .is_some_and(|m| m.starts_with("notifications/"));
                if !is_notification {
                    let resp = JsonRpcResponse::error(
                        Value::Null,
                        ErrorCode::InvalidRequest,
                        "Missing `id` in request.",
                    );
                    Self::write_response(&mut writer, &resp);
                    continue;
                }
            }

            if let Some(resp) = self.process_request(&request, &id) {
                Self::write_response(&mut writer, &resp);
            }
        }

        std::process::exit(0);
    }

    fn write_response(writer: &mut impl Write, resp: &JsonRpcResponse) {
        let json = serde_json::to_string(resp).expect("failed to serialize response");
        let _ = writeln!(writer, "{json}");
        let _ = writer.flush();
    }

    fn process_request(&self, request: &JsonRpcRequest, id: &Value) -> Option<JsonRpcResponse> {
        let method = match &request.method {
            Some(m) => m.as_str(),
            None => {
                return Some(JsonRpcResponse::error(
                    id.clone(),
                    ErrorCode::InvalidRequest,
                    "Missing `method` key in JSON request.",
                ));
            }
        };

        if self.verbose {
            eprintln!("Processing method: {method}");
        }

        match method {
            "initialize" => Some(self.handle_initialize(id)),
            "tools/list" => Some(self.handle_tools_list(id)),
            "tools/call" => Some(self.handle_tools_call(id, request)),
            "resources/list" => Some(self.handle_resources_list(id)),
            "resources/read" => Some(self.handle_resources_read(id, request)),
            "ping" => Some(JsonRpcResponse::success(id.clone(), json!({}))),
            m if m.starts_with("notifications/") => {
                self.handle_notification(m, request.params.as_ref());
                None
            }
            _ => Some(JsonRpcResponse::error(
                id.clone(),
                ErrorCode::MethodNotFound,
                format!("Method not found: {method}"),
            )),
        }
    }

    fn handle_initialize(&self, id: &Value) -> JsonRpcResponse {
        let result = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        });
        JsonRpcResponse::success(id.clone(), result)
    }

    fn handle_tools_list(&self, id: &Value) -> JsonRpcResponse {
        let tools: Vec<Value> = self.tools.iter().map(|t| tool_to_json(t.as_ref())).collect();
        JsonRpcResponse::success(id.clone(), json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, id: &Value, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id.clone(),
                    ErrorCode::InvalidRequest,
                    "Missing `params` key in request.",
                );
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    id.clone(),
                    ErrorCode::InvalidRequest,
                    "Missing tool `name` in params.",
                );
            }
        };

        let tool = match self.tools.iter().find(|t| t.name() == tool_name) {
            Some(t) => t,
            None => {
                return JsonRpcResponse::error(
                    id.clone(),
                    ErrorCode::MethodNotFound,
                    format!("There is no tool named `{tool_name}`."),
                );
            }
        };

        let arguments = match params.get("arguments") {
            Some(Value::Object(map)) => map
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<std::collections::HashMap<String, Value>>(),
            Some(_) => {
                return JsonRpcResponse::error(
                    id.clone(),
                    ErrorCode::InvalidParams,
                    "The `arguments` value is not a valid object.",
                );
            }
            None => std::collections::HashMap::new(),
        };

        if let Err(msg) = validate_arguments(tool.as_ref(), &arguments) {
            if self.verbose {
                eprintln!("{msg}");
            }
            return JsonRpcResponse::error(id.clone(), ErrorCode::InvalidParams, msg);
        }

        if self.verbose {
            eprintln!("Calling tool: {tool_name}");
        }

        let ctx = ToolContext {
            ide: self.ide.as_ref(),
            docs_path: self.docs_path.as_deref(),
            verbose: self.verbose,
        };

        let result = tool.run(&arguments, &ctx);
        self.tool_response(id, &result)
    }

    fn tool_response(&self, id: &Value, result: &ToolResult) -> JsonRpcResponse {
        let mut res = json!({
            "content": [{
                "type": "text",
                "text": result.output
            }]
        });
        if result.is_error {
            res.as_object_mut()
                .unwrap()
                .insert("isError".into(), Value::Bool(true));
        }
        JsonRpcResponse::success(id.clone(), res)
    }

    fn handle_resources_list(&self, id: &Value) -> JsonRpcResponse {
        let result = resources::resources_list(&self.exe_dir);
        JsonRpcResponse::success(id.clone(), result)
    }

    fn handle_resources_read(&self, id: &Value, request: &JsonRpcRequest) -> JsonRpcResponse {
        let uri = request
            .params
            .as_ref()
            .and_then(|p| p.get("uri"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match resources::resources_read(uri, &self.exe_dir) {
            Ok(result) => JsonRpcResponse::success(id.clone(), result),
            Err(msg) => JsonRpcResponse::error(id.clone(), ErrorCode::InvalidParams, msg),
        }
    }

    fn handle_notification(&self, method: &str, params: Option<&Value>) {
        if !self.verbose {
            return;
        }
        let kind = method.strip_prefix("notifications/").unwrap_or(method);
        match kind {
            "initialized" => eprintln!("MCP Client successfully initialised."),
            "cancelled" => {
                let req_id = params
                    .and_then(|p| p.get("requestId"))
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "null".into());
                eprintln!("The MCP client wants to cancel request {req_id}.");
            }
            "progress" => eprintln!("The MCP server has reported progress."),
            "roots/list_changed" => eprintln!("`roots/list_changed` notification received."),
            _ => eprintln!("Unknown MCP client notification received."),
        }
    }
}
