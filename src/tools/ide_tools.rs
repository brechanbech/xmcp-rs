use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::ide::script::{build_string_variable_script, escape_ide_string, indent_lines};
use crate::mcp::tool::*;
use crate::tools::*;

// ---------------------------------------------------------------------------
// list_project_items
// ---------------------------------------------------------------------------
pub struct ListProjectItems;

impl Tool for ListProjectItems {
    fn name(&self) -> &'static str { "list_project_items" }
    fn description(&self) -> &'static str {
        "Lists child items at a given project location in the Xojo IDE Navigator. \
         Returns tab-delimited list. Empty location = top-level items."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "location",
            param_type: ParamType::String,
            description: "Dot-separated project path (e.g. 'App' or 'Module1.Method1'). Leave empty for top-level items.",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let location = arg_str(args, "location", "");
        let escaped = escape_ide_string(location);
        let script = format!(r#"Print SubLocations("{escaped}")"#);
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// get_current_location
// ---------------------------------------------------------------------------
pub struct GetCurrentLocation;

impl Tool for GetCurrentLocation {
    fn name(&self) -> &'static str { "get_current_location" }
    fn description(&self) -> &'static str {
        "Returns the currently selected location in the Xojo IDE Navigator and its type \
         (e.g. Class, Method, Window)."
    }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = r#"Dim loc As String = Location
Dim typ As String = TypeOfCurrentLocation
Print loc + " (" + typ + ")""#;
        ide_call_default(ctx, script)
    }
}

// ---------------------------------------------------------------------------
// select_project_item
// ---------------------------------------------------------------------------
pub struct SelectProjectItem;

impl Tool for SelectProjectItem {
    fn name(&self) -> &'static str { "select_project_item" }
    fn description(&self) -> &'static str {
        "Selects and navigates to a specific item in the Xojo IDE Navigator. \
         Use dot-separated paths."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "item_path",
            param_type: ParamType::String,
            description: "Dot-separated path (e.g. 'App', 'Module1.MyMethod')",
            required: true,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let path = arg_str(args, "item_path", "");
        let escaped = escape_ide_string(path);
        let script = format!(
            r#"Dim result As Boolean = SelectProjectItem("{escaped}")
If result Then
  Print "Selected: " + Location + " (" + TypeOfCurrentLocation + ")"
Else
  Print "ERROR: Could not select '{escaped}'. Method-level items auto-navigate when using get_code/set_code. For window event handlers, edit the .xojo_window file directly and use revert_project."
End If"#
        );
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// get_code
// ---------------------------------------------------------------------------
pub struct GetCode;

impl Tool for GetCode {
    fn name(&self) -> &'static str { "get_code" }
    fn description(&self) -> &'static str {
        "Reads the source code at the current or specified location in the Xojo IDE."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "location",
            param_type: ParamType::String,
            description: "Dot-separated path (e.g. 'Module1.Method1'). Reads current location if empty.",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let location = arg_str(args, "location", "");
        let script = if location.is_empty() {
            "Try\n  Print Text\nCatch\n  Print \"ERROR: No code editor is active. Try navigating to a code item first, or edit the .xojo_code file directly.\"\nEnd Try".to_string()
        } else {
            let escaped = escape_ide_string(location);
            format!(
                r#"If SelectProjectItem("{escaped}") Then
  Try
    Print Text
  Catch
    Print "ERROR: No code editor is active for '{escaped}'. If this is a window event handler, edit the .xojo_window file directly and use revert_project."
  End Try
Else
  Print "ERROR: Could not select '{escaped}'."
End If"#
            )
        };
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// set_code
// ---------------------------------------------------------------------------
pub struct SetCode;

impl Tool for SetCode {
    fn name(&self) -> &'static str { "set_code" }
    fn description(&self) -> &'static str {
        "Writes source code to the current or specified location in the Xojo IDE. \
         Replaces entire code content."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "code",
            param_type: ParamType::String,
            description: "Source code to write",
            required: true,
            default: None,
        }, ToolParam {
            name: "location",
            param_type: ParamType::String,
            description: "Dot-separated path to navigate to first",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let code = arg_str(args, "code", "");
        let location = arg_str(args, "location", "");

        let var_script = build_string_variable_script("__code", code);
        let write_part = "Text = __code\nPrint \"Code written to: \" + Location";

        let script = if location.is_empty() {
            let inner = indent_lines(write_part, "  ");
            format!("{var_script}\nTry\n{inner}\nCatch\n  Print \"ERROR: No code editor is active. If this is a window event handler, edit the .xojo_window file directly and use revert_project.\"\nEnd Try")
        } else {
            let escaped = escape_ide_string(location);
            let inner = indent_lines(write_part, "    ");
            format!(
                "{var_script}\nIf SelectProjectItem(\"{escaped}\") Then\n  Try\n{inner}\n  Catch\n    Print \"ERROR: No code editor is active for '{escaped}'. If this is a window event handler, edit the .xojo_window file directly and use revert_project.\"\n  End Try\nElse\n  Print \"ERROR: Could not select '{escaped}'.\"\nEnd If"
            )
        };
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// get_selected_text
// ---------------------------------------------------------------------------
pub struct GetSelectedText;

impl Tool for GetSelectedText {
    fn name(&self) -> &'static str { "get_selected_text" }
    fn description(&self) -> &'static str {
        "Returns the currently selected text in the Xojo IDE code editor, \
         along with selection start position and length."
    }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = r#"Try
  Dim s As String = SelectedText
  Dim ss As Integer = SelectionStart
  Dim sl As Integer = SelectionLength
  Print "start=" + Str(ss) + " length=" + Str(sl) + Chr(10) + s
Catch
  Print "ERROR: No code editor is active."
End Try"#;
        ide_call_default(ctx, script)
    }
}

// ---------------------------------------------------------------------------
// set_selected_text
// ---------------------------------------------------------------------------
pub struct SetSelectedText;

impl Tool for SetSelectedText {
    fn name(&self) -> &'static str { "set_selected_text" }
    fn description(&self) -> &'static str {
        "Replaces the currently selected text in the Xojo IDE code editor with new text. \
         Can optionally set selection position first."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "text",
            param_type: ParamType::String,
            description: "Replacement text to insert",
            required: true,
            default: None,
        }, ToolParam {
            name: "selection_start",
            param_type: ParamType::Integer,
            description: "Character offset to set before replacing. Skipped if -1.",
            required: false,
            default: None,
        }, ToolParam {
            name: "selection_length",
            param_type: ParamType::Integer,
            description: "Characters to select before replacing",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let text = arg_str(args, "text", "");
        let sel_start = arg_i64(args, "selection_start", -1);
        let sel_length = arg_i64(args, "selection_length", 0);

        let var_script = build_string_variable_script("__text", text);

        let mut inner = String::new();
        if sel_start >= 0 {
            inner.push_str(&format!("  SelectionStart = {sel_start}\n"));
            inner.push_str(&format!("  SelectionLength = {sel_length}\n"));
        }
        inner.push_str("  SelectedText = __text\n");
        inner.push_str("  Print \"Text replaced successfully.\"");

        let script = format!(
            "{var_script}\nTry\n{inner}\nCatch\n  Print \"ERROR: No code editor is active.\"\nEnd Try"
        );
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// build_project
// ---------------------------------------------------------------------------
pub struct BuildProject;

impl Tool for BuildProject {
    fn name(&self) -> &'static str { "build_project" }
    fn description(&self) -> &'static str {
        "Builds the current Xojo project. Returns the path to the built application \
         on success, or build errors on failure."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "build_type",
            param_type: ParamType::Integer,
            description: "Build target type (0=Default, 5=macOS Cocoa, 9=Windows 32-bit, \
                          14=Windows 64-bit, 16=Linux 32-bit, 17=Linux 64-bit, 18=Linux ARM, \
                          24=macOS Universal).",
            required: false,
            default: None,
        }, ToolParam {
            name: "reveal",
            param_type: ParamType::Boolean,
            description: "Whether to reveal built app in Finder/Explorer.",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let build_type = arg_i64(args, "build_type", 0);
        let reveal = arg_bool(args, "reveal", false);
        let reveal_str = if reveal { "True" } else { "False" };
        let script = format!("DoCommand \"BuildApp {build_type} {reveal_str}\"\nPrint \"\"");
        let result = ide_call(ctx, &script, Duration::from_secs(120));
        if result.is_error {
            return result;
        }
        parse_do_command_result(&result.output, "Build succeeded.")
    }
}

// ---------------------------------------------------------------------------
// run_project
// ---------------------------------------------------------------------------
pub struct RunProject;

impl Tool for RunProject {
    fn name(&self) -> &'static str { "run_project" }
    fn description(&self) -> &'static str { "Runs the current Xojo project in debug mode." }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = "DoCommand \"RunApp\"\nPrint \"\"";
        let result = ide_call(ctx, script, Duration::from_secs(30));
        if result.is_error {
            return result;
        }
        parse_do_command_result(&result.output, "Project launched in debug mode.")
    }
}

// ---------------------------------------------------------------------------
// stop_project
// ---------------------------------------------------------------------------
pub struct StopProject;

impl Tool for StopProject {
    fn name(&self) -> &'static str { "stop_project" }
    fn description(&self) -> &'static str { "Stops the currently running Xojo debug session." }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = "DoCommand \"Kill\"\nPrint \"Debug session stopped.\"";
        ide_call_default(ctx, script)
    }
}

// ---------------------------------------------------------------------------
// create_project_item
// ---------------------------------------------------------------------------
pub struct CreateProjectItem;

const VALID_ITEM_TYPES: &[&str] = &[
    "NewClass", "NewModule", "NewMethod", "NewProperty", "NewConstant",
    "NewEvent", "NewNote", "NewMenuHandler", "NewComputedProperty",
    "NewSharedMethod", "NewSharedProperty", "NewEnum", "NewStructure",
    "NewDelegate", "NewInterface", "NewWindow", "NewContainerControl",
    "NewFolder", "AddEventImplementation",
];

impl Tool for CreateProjectItem {
    fn name(&self) -> &'static str { "create_project_item" }
    fn description(&self) -> &'static str {
        "Creates a new project item in the Xojo IDE. First navigates to the target \
         location (if specified), then creates the item."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "item_type",
            param_type: ParamType::String,
            description: "Item type to create. Must be one of: NewClass, NewModule, NewMethod, \
                          NewProperty, NewConstant, NewEvent, NewNote, NewMenuHandler, \
                          NewComputedProperty, NewSharedMethod, NewSharedProperty, NewEnum, \
                          NewStructure, NewDelegate, NewInterface, NewWindow, \
                          NewContainerControl, NewFolder, AddEventImplementation",
            required: true,
            default: None,
        }, ToolParam {
            name: "parent_location",
            param_type: ParamType::String,
            description: "Dot-separated path to navigate to first (e.g. 'Module1')",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let item_type = arg_str(args, "item_type", "");
        let parent = arg_str(args, "parent_location", "");

        if !VALID_ITEM_TYPES.contains(&item_type) {
            return ToolResult::failure(format!(
                "Invalid item_type '{item_type}'. Must be one of: {}",
                VALID_ITEM_TYPES.join(", ")
            ));
        }

        let script = if parent.is_empty() {
            format!(
                "DoCommand \"{item_type}\"\nPrint \"Created {item_type} at: \" + Location"
            )
        } else {
            let escaped = escape_ide_string(parent);
            format!(
                "If SelectProjectItem(\"{escaped}\") Then\n  DoCommand \"{item_type}\"\n  Print \"Created {item_type} at: \" + Location\nElse\n  Print \"ERROR: Could not select '{escaped}'.\"\nEnd If"
            )
        };
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// run_ide_script
// ---------------------------------------------------------------------------
pub struct RunIdeScript;

impl Tool for RunIdeScript {
    fn name(&self) -> &'static str { "run_ide_script" }
    fn description(&self) -> &'static str {
        "Executes an arbitrary Xojo IDE script. Use Print to return values. \
         Powerful escape hatch — IDE scripting can quit Xojo, close/reopen \
         projects, create or delete items, write files, and invoke shell \
         commands via DoShellCommand. Prefer a dedicated tool when one fits \
         (list_project_items, get_code, build_project, etc.); use this only \
         when nothing else does, and pause before running anything that \
         quits the IDE, deletes items, or touches the shell."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "script",
            param_type: ParamType::String,
            description: "IDE script code (XojoScript syntax). Use Print for output.",
            required: true,
            default: None,
        }, ToolParam {
            name: "timeout",
            param_type: ParamType::Integer,
            description: "Timeout in milliseconds (default 10000)",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = arg_str(args, "script", "");
        let timeout_ms = arg_i64(args, "timeout", 10000) as u64;
        ide_call(ctx, script, Duration::from_millis(timeout_ms))
    }
}

// ---------------------------------------------------------------------------
// get_project_info
// ---------------------------------------------------------------------------
pub struct GetProjectInfo;

impl Tool for GetProjectInfo {
    fn name(&self) -> &'static str { "get_project_info" }
    fn description(&self) -> &'static str {
        "Returns information about the currently open Xojo project including path, \
         Xojo IDE version, and selected item."
    }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = r#"Dim info As String = ""
info = info + "Project: " + ProjectShellPath + Chr(10)
info = info + "Xojo Version: " + Str(XojoVersion) + Chr(10)
info = info + "Current Location: " + Location + Chr(10)
info = info + "Location Type: " + TypeOfCurrentLocation + Chr(10)
info = info + "Selected Item: " + ProjectItem
Print info"#;
        let result = ide_call_default(ctx, script);
        if result.is_error {
            return result;
        }
        // Post-process: derive project directory from the Project: line.
        let mut output = result.output.clone();
        for line in result.output.lines() {
            if let Some(path) = line.strip_prefix("Project: ") {
                let path = std::path::Path::new(path.trim());
                if let Some(parent) = path.parent() {
                    output.push('\n');
                    output.push_str(&format!("Project Directory: {}", parent.display()));
                }
                break;
            }
        }
        ToolResult::success(output)
    }
}

// ---------------------------------------------------------------------------
// revert_project
// ---------------------------------------------------------------------------
pub struct RevertProject;

impl Tool for RevertProject {
    fn name(&self) -> &'static str { "revert_project" }
    fn description(&self) -> &'static str {
        "Reverts the current Xojo project to the version saved on disk. \
         Use after modifying project files directly."
    }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let script = r#"Dim path As String = ProjectShellPath
CloseProject(False)
OpenFile path
Print "Project reloaded from disk.""#;
        ide_call(ctx, script, Duration::from_secs(15))
    }
}

// ---------------------------------------------------------------------------
// save_project
// ---------------------------------------------------------------------------
pub struct SaveProject;

impl Tool for SaveProject {
    fn name(&self) -> &'static str { "save_project" }
    fn description(&self) -> &'static str {
        "Saves the current Xojo project to disk via IDE scripting \
         (DoCommand \"SaveFile\")."
    }
    fn parameters(&self) -> &[ToolParam] { &[] }
    fn run(&self, _args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let path_result = ide_call_default(ctx, "Print ProjectShellPath");
        if path_result.is_error {
            return path_result;
        }
        let project_path = path_result.output.trim().to_string();
        if project_path.is_empty() {
            return ToolResult::failure(
                "ProjectShellPath was empty — is a project open in the Xojo IDE?",
            );
        }
        let file_path = Path::new(&project_path).to_path_buf();
        let dir_path = file_path.parent().map(|p| p.to_path_buf());

        let baseline = (mtime(&file_path), dir_path.as_deref().and_then(mtime));

        // "SaveFile" is the documented DoCommand that saves the whole project with
        // no prompt; despite the name it is project-wide, not per-file.
        let ide_save = ide_call_default(ctx, "DoCommand \"SaveFile\"\nPrint \"Saved\"");
        if ide_save.is_error {
            return ide_save;
        }

        if mtime_changed(&file_path, dir_path.as_deref(), baseline, Duration::from_millis(2000)) {
            ToolResult::success("Project saved.")
        } else {
            ToolResult::success("No changes to save.")
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn mtime_changed(
    file: &Path,
    dir: Option<&Path>,
    baseline: (Option<SystemTime>, Option<SystemTime>),
    budget: Duration,
) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        let file_now = mtime(file);
        let dir_now = dir.and_then(mtime);
        if changed(file_now, baseline.0) || changed(dir_now, baseline.1) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn changed(now: Option<SystemTime>, before: Option<SystemTime>) -> bool {
    match (now, before) {
        (Some(n), Some(b)) => n > b,
        (Some(_), None) => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// get_item_description
// ---------------------------------------------------------------------------
pub struct GetItemDescription;

impl Tool for GetItemDescription {
    fn name(&self) -> &'static str { "get_item_description" }
    fn description(&self) -> &'static str {
        "Gets or sets the description of the currently selected project item \
         (method, property, event, etc.)."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "location",
            param_type: ParamType::String,
            description: "Dot-separated path to navigate to first",
            required: false,
            default: None,
        }, ToolParam {
            name: "value",
            param_type: ParamType::String,
            description: "If provided, sets description; if omitted, reads it",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let location = arg_str(args, "location", "");
        let has_value = args.contains_key("value");
        let value = arg_str(args, "value", "");

        let core = if has_value {
            let escaped_val = escape_ide_string(value);
            format!("ItemDescription = \"{escaped_val}\"\nPrint \"OK\"")
        } else {
            "Print ItemDescription".to_string()
        };

        let script = if location.is_empty() {
            core
        } else {
            let escaped_loc = escape_ide_string(location);
            format!(
                "If SelectProjectItem(\"{escaped_loc}\") Then\n  {core}\nElse\n  Print \"ERROR: Could not select '{escaped_loc}'.\"\nEnd If"
            )
        };
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// constant_value
// ---------------------------------------------------------------------------
pub struct ConstantValue;

impl Tool for ConstantValue {
    fn name(&self) -> &'static str { "constant_value" }
    fn description(&self) -> &'static str {
        "Gets or sets the value of a project constant in the Xojo IDE. \
         The constant must already exist in the project."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "name",
            param_type: ParamType::String,
            description: "Simple or fully qualified constant name (e.g. 'kVersion' or 'App.kVersion')",
            required: true,
            default: None,
        }, ToolParam {
            name: "value",
            param_type: ParamType::String,
            description: "If provided, sets the constant; if omitted, returns current value",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let name = arg_str(args, "name", "");
        let has_value = args.contains_key("value");
        let value = arg_str(args, "value", "");
        let escaped_name = escape_ide_string(name);

        let script = if has_value {
            let escaped_val = escape_ide_string(value);
            format!("ConstantValue(\"{escaped_name}\") = \"{escaped_val}\"\nPrint \"OK\"")
        } else {
            format!("Print ConstantValue(\"{escaped_name}\")")
        };
        ide_call_default(ctx, &script)
    }
}

// ---------------------------------------------------------------------------
// Shared helper: parse DoCommand result for build/run
// ---------------------------------------------------------------------------
fn parse_do_command_result(output: &str, success_msg: &str) -> ToolResult {
    let trimmed = output.trim();

    // Empty or just "{}" means success.
    if trimmed.is_empty() || trimmed == "{}" {
        return ToolResult::success(success_msg);
    }

    // Try parsing as JSON to extract build errors.
    if let Ok(json) = serde_json::from_str::<Value>(trimmed)
        && let Some(errors) = json
            .get("buildError")
            .and_then(|be| be.get("errors"))
            .and_then(|e| e.as_array())
    {
        let mut msg = String::from("Build failed with errors:\n");
        for err in errors {
            let err_type = err.get("type").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let err_msg = err.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let err_loc = err.get("location").and_then(|v| v.as_str()).unwrap_or("");
            let err_pos = err.get("position").and_then(|v| v.as_i64()).unwrap_or(-1);
            msg.push_str(&format!("  [{err_type}] {err_msg}"));
            if !err_loc.is_empty() {
                msg.push_str(&format!(" at {err_loc}"));
            }
            if err_pos >= 0 {
                msg.push_str(&format!(" (position {err_pos})"));
            }
            msg.push('\n');
        }
        return ToolResult::failure(msg);
    }

    // If it's not JSON or doesn't have buildError, return as-is.
    ToolResult::success(trimmed)
}
