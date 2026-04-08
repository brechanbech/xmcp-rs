# XMCP-rs

An MCP (Model Context Protocol) server that gives AI assistants direct control
over the Xojo IDE. Communicates via stdin/stdout JSON-RPC and forwards IDE
commands via a Unix domain socket to the running Xojo IDE process.

## Attribution

This is a Rust port of [XMCP](https://github.com/o3jvind/XMCP) by
Øjvind Søgaard Andersen, originally written in Xojo. The original project
is licensed under the MIT License.

## Quick start

### 1. Build and install

```sh
git clone https://codeberg.org/brechanbech/XMCP-rs.git
cd XMCP-rs
cargo install --path .
```

This installs the `xmcp` binary to `~/.cargo/bin/xmcp`. If `~/.cargo/bin`
is already on your `PATH` (the Rust installer adds it by default), you're
done. Verify with:

```sh
xmcp --help
```

You also need to place `usage-guide.md` next to the binary so that MCP
clients can fetch it as a resource:

```sh
cp usage-guide.md ~/.cargo/bin/
```

### 2. Add to Claude Code

Run this from any terminal:

```sh
claude mcp add xmcp -- xmcp
```

Or add it manually to your Claude Code settings. Open the MCP config file
(on macOS: `~/.claude/settings.json` or the project-level
`.claude/settings.json`) and add:

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

To enable verbose logging (written to stderr, visible in the Claude Code
MCP log):

```json
{
  "mcpServers": {
    "xmcp": {
      "command": "xmcp",
      "args": ["-v"]
    }
  }
}
```

### 3. Use it

1. Start the Xojo IDE and open your project
2. Start a Claude Code session in the project directory
3. Claude will automatically discover the 22 XMCP tools and the usage guide

## Requirements

- macOS (the Xojo IDE IPC socket is macOS-specific)
- Rust toolchain (`rustup` — https://rustup.rs)
- Xojo IDE must be running with a project open before using any tools

## Options

```
xmcp [OPTIONS]
```

- `-v`, `--verbose` — Enable verbose logging to stderr
- `-d`, `--docs-path <PATH>` — Path to Xojo documentation directory (auto-detected if omitted)
- `-h`, `--help` — Print help

## Differences from the original XMCP

This is a faithful port — same MCP protocol version (`2025-06-18`), same 22
tools with identical names and parameters, same IDE Communicator Protocol v2
over the Unix domain socket. It is a drop-in replacement.

Notable differences:

- **Binary name** is `xmcp` (lowercase) rather than `XMCP`
- **No Xojo license required** — builds with the standard Rust toolchain
- **usage-guide.md has a compiled-in fallback** — the original fails silently
  if the file is missing next to the binary; the Rust version embeds a copy
  at compile time so the MCP resource is always available. A file on disk
  still takes priority, so you can edit it without rebuilding.
- **CLI parsing** uses [clap](https://crates.io/crates/clap) rather than the
  original's custom OptionParser. The flags are the same.

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
