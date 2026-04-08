use std::collections::HashMap;
use std::process::Command;

use serde_json::Value;

use crate::mcp::tool::*;
use crate::tools::*;

const DEBUG_LOG_PATH: &str = "/tmp/xmcp_debug.log";

// ---------------------------------------------------------------------------
// get_debug_log
// ---------------------------------------------------------------------------
pub struct GetDebugLog;

impl Tool for GetDebugLog {
    fn name(&self) -> &'static str { "get_debug_log" }
    fn description(&self) -> &'static str {
        "Reads the XMCP debug log file at /tmp/xmcp_debug.log \
         (written by App.UnhandledException handlers). Returns exception details \
         or empty message if no log exists."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "clear",
            param_type: ParamType::Boolean,
            description: "Delete log file after reading",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, _ctx: &ToolContext) -> ToolResult {
        let clear = arg_bool(args, "clear", false);
        let path = std::path::Path::new(DEBUG_LOG_PATH);

        if !path.exists() {
            return ToolResult::success(
                "No debug log found at /tmp/xmcp_debug.log. \
                 This file is created when an app with the UnhandledException handler crashes.",
            );
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return ToolResult::failure(format!("Could not read debug log: {e}")),
        };

        if clear {
            let _ = std::fs::remove_file(path);
        }

        if content.is_empty() {
            ToolResult::success("Debug log exists but is empty.")
        } else {
            ToolResult::success(content)
        }
    }
}

// ---------------------------------------------------------------------------
// get_system_log
// ---------------------------------------------------------------------------
pub struct GetSystemLog;

impl Tool for GetSystemLog {
    fn name(&self) -> &'static str { "get_system_log" }
    fn description(&self) -> &'static str {
        "Reads recent System.DebugLog output from the macOS unified log for a running \
         Xojo debug app (process name = app name + '.debug')."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "process_name",
            param_type: ParamType::String,
            description: "Process name filter (e.g. 'MyApp.debug')",
            required: true,
            default: None,
        }, ToolParam {
            name: "seconds",
            param_type: ParamType::Integer,
            description: "How many seconds back to search (default 60, max 3600)",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, _ctx: &ToolContext) -> ToolResult {
        let process_name = arg_str(args, "process_name", "");
        let seconds = arg_i64(args, "seconds", 60).clamp(1, 3600);

        // Input sanitisation: only allow safe characters in process name.
        if !process_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == ' ' || c == '-')
        {
            return ToolResult::failure(
                "Invalid process_name. Only alphanumeric characters, underscores, dots, \
                 hyphens, and spaces are allowed.",
            );
        }

        if process_name.is_empty() {
            return ToolResult::failure("The `process_name` parameter cannot be empty.");
        }

        let output = match Command::new("log")
            .args([
                "show",
                "--last",
                &format!("{seconds}s"),
                "--predicate",
                &format!("process == \"{process_name}\""),
            ])
            .stderr(std::process::Stdio::null())
            .output()
        {
            Ok(o) => o,
            Err(e) => return ToolResult::failure(format!("Failed to run `log show`: {e}")),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Filter for XojoFramework sender lines (System.DebugLog output).
        let filtered: Vec<&str> = stdout
            .lines()
            .filter(|line| line.contains("(XojoFramework)"))
            .collect();

        if filtered.is_empty() {
            ToolResult::success(format!(
                "No log entries found for process '{process_name}' in the last {seconds} seconds."
            ))
        } else {
            ToolResult::success(filtered.join("\n"))
        }
    }
}
