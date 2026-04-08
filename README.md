# XMCP-rs

An MCP (Model Context Protocol) server that gives AI assistants direct control
over the Xojo IDE. Communicates via stdin/stdout JSON-RPC and forwards IDE
commands via a Unix domain socket to the running Xojo IDE process.

## Attribution

This is a Rust port of [XMCP](https://github.com/o3jvind/XMCP) by
Øjvind Søgaard Andersen, originally written in Xojo. The original project
is licensed under the MIT License.

## Building

```sh
cargo install --path .
```

The binary is installed to `~/.cargo/bin/xmcp`.

## Usage

```sh
xmcp [OPTIONS]
```

### Options

- `-v`, `--verbose` — Enable verbose logging to stderr
- `-d`, `--docs-path <PATH>` — Path to Xojo documentation directory (auto-detected if omitted)

### Requirements

- macOS (Unix domain socket to Xojo IDE)
- Xojo IDE must be running with a project open

### Claude Code integration

Add to your MCP configuration:

```json
{
  "mcpServers": {
    "xmcp": {
      "command": "xmcp",
      "args": []
    }
  }
}
```

## Tools

XMCP exposes 22 tools across four categories:

**IDE tools (16):** list_project_items, get_current_location, select_project_item,
get_code, set_code, get_selected_text, set_selected_text, build_project,
run_project, stop_project, create_project_item, run_ide_script, get_project_info,
revert_project, get_item_description, constant_value

**Documentation tools (3):** search_docs, lookup_class, list_doc_topics

**Debug tools (2):** get_debug_log, get_system_log

**Cost awareness (1):** estimate_request_cost

## License

MIT — see [LICENSE](LICENSE) for details.
