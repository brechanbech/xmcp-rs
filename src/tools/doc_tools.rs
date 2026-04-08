use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

use crate::mcp::tool::*;
use crate::tools::*;

// ---------------------------------------------------------------------------
// search_docs
// ---------------------------------------------------------------------------
pub struct SearchDocs {
    cached_lines: OnceLock<Vec<String>>,
}

impl SearchDocs {
    pub fn new() -> Self {
        Self {
            cached_lines: OnceLock::new(),
        }
    }

    fn load_lines(&self, docs_path: &Path) -> &[String] {
        self.cached_lines.get_or_init(|| {
            let file = docs_path.join("llms-full.txt");
            match std::fs::read_to_string(&file) {
                Ok(content) => content.lines().map(String::from).collect(),
                Err(_) => Vec::new(),
            }
        })
    }
}

impl Tool for SearchDocs {
    fn name(&self) -> &'static str { "search_docs" }
    fn description(&self) -> &'static str {
        "Searches local Xojo documentation guides and tutorials by keyword. Returns matching \
         sections with context. For specific class/method lookup, use lookup_class instead."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "query",
            param_type: ParamType::String,
            description: "Search term (e.g. 'JSONItem', 'database')",
            required: true,
            default: None,
        }, ToolParam {
            name: "max_results",
            param_type: ParamType::Integer,
            description: "Max sections to return",
            required: false,
            default: None,
        }, ToolParam {
            name: "context_lines",
            param_type: ParamType::Integer,
            description: "Lines before/after match to include",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let docs_path = match ctx.docs_path {
            Some(p) => p,
            None => return ToolResult::failure("Xojo documentation path not configured."),
        };

        let query = arg_str(args, "query", "");
        let max_results = arg_i64(args, "max_results", 5) as usize;
        let context_lines = arg_i64(args, "context_lines", 10) as usize;

        if query.is_empty() {
            return ToolResult::failure("The `query` parameter cannot be empty.");
        }

        let lines = self.load_lines(docs_path);
        if lines.is_empty() {
            return ToolResult::failure("Documentation not loaded. Check docs path.");
        }

        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        let mut current_section = String::new();
        let mut last_end: usize = 0;

        for (i, line) in lines.iter().enumerate() {
            if line.starts_with('#') {
                current_section = line.clone();
            }

            if line.to_lowercase().contains(&query_lower) {
                // Skip if within previous context window.
                if i < last_end {
                    continue;
                }

                let start = i.saturating_sub(context_lines);
                let end = (i + context_lines + 1).min(lines.len());
                last_end = end;

                let mut section_output = String::new();
                if !current_section.is_empty() {
                    section_output.push_str(&format!("--- {current_section} ---\n"));
                }

                for (j, line) in lines.iter().enumerate().take(end).skip(start) {
                    if j == i {
                        section_output.push_str(&format!(">>> {line}\n"));
                    } else {
                        section_output.push_str(&format!("    {line}\n"));
                    }
                }

                results.push(section_output);
                if results.len() >= max_results {
                    break;
                }
            }
        }

        if results.is_empty() {
            ToolResult::success(format!("No matches found for '{query}'."))
        } else {
            ToolResult::success(results.join("\n"))
        }
    }
}

// ---------------------------------------------------------------------------
// lookup_class
// ---------------------------------------------------------------------------
pub struct LookupClass;

impl Tool for LookupClass {
    fn name(&self) -> &'static str { "lookup_class" }
    fn description(&self) -> &'static str {
        "Looks up detailed documentation for a specific Xojo class, control, data type, or API by name."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "class_name",
            param_type: ParamType::String,
            description: "Class name to look up (e.g. 'DesktopButton', 'JSONItem')",
            required: true,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let docs_path = match ctx.docs_path {
            Some(p) => p,
            None => return ToolResult::failure("Xojo documentation path not configured."),
        };

        let class_name = arg_str(args, "class_name", "");
        if class_name.is_empty() {
            return ToolResult::failure("The `class_name` parameter cannot be empty.");
        }

        let sources_dir = docs_path.join("_sources");
        if !sources_dir.exists() {
            return ToolResult::failure("Documentation _sources directory not found.");
        }

        let lower = class_name.to_lowercase();
        let candidates = [
            format!("{lower}.rst.txt"),
            format!("desktop{lower}.rst.txt"),
            format!("web{lower}.rst.txt"),
        ];

        for candidate in &candidates {
            if let Some(path) = find_file_recursive(&sources_dir, candidate) {
                match std::fs::read_to_string(&path) {
                    Ok(content) => return ToolResult::success(content),
                    Err(e) => {
                        return ToolResult::failure(format!(
                            "Could not read {}: {e}",
                            path.display()
                        ))
                    }
                }
            }
        }

        ToolResult::failure(format!(
            "No documentation found for '{class_name}'. Try search_docs instead."
        ))
    }
}

fn find_file_recursive(dir: &Path, target: &str) -> Option<std::path::PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, target) {
                return Some(found);
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.to_lowercase() == target
        {
            return Some(path);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// list_doc_topics
// ---------------------------------------------------------------------------
pub struct ListDocTopics;

impl Tool for ListDocTopics {
    fn name(&self) -> &'static str { "list_doc_topics" }
    fn description(&self) -> &'static str {
        "Lists available Xojo documentation topics and pages. Optionally filtered by keyword."
    }
    fn parameters(&self) -> &[ToolParam] {
        static P: &[ToolParam] = &[ToolParam {
            name: "filter",
            param_type: ParamType::String,
            description: "Keyword to filter topics (e.g. 'Desktop', 'database'). Returns all if empty.",
            required: false,
            default: None,
        }];
        P
    }
    fn run(&self, args: &HashMap<String, Value>, ctx: &ToolContext) -> ToolResult {
        let docs_path = match ctx.docs_path {
            Some(p) => p,
            None => return ToolResult::failure("Xojo documentation path not configured."),
        };

        let filter = arg_str(args, "filter", "");
        let file = docs_path.join("llms.txt");

        let content = match std::fs::read_to_string(&file) {
            Ok(c) => c,
            Err(e) => return ToolResult::failure(format!("Could not read llms.txt: {e}")),
        };

        if filter.is_empty() {
            return ToolResult::success(content);
        }

        let filter_lower = filter.to_lowercase();
        let matched: Vec<&str> = content
            .lines()
            .filter(|line| line.to_lowercase().contains(&filter_lower))
            .collect();

        if matched.is_empty() {
            ToolResult::success(format!("No topics found matching: {filter}"))
        } else {
            ToolResult::success(format!(
                "{} topics matching '{filter}':\n{}",
                matched.len(),
                matched.join("\n")
            ))
        }
    }
}
