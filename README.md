# org-management

A CLI and MCP server for tracking agent hierarchy, relationships, and resource artifacts in multi-agent systems. Stores data in a local SQLite database with WAL mode for concurrent access.

## Install

```bash
cargo install --path .
```

Or via Homebrew:

```bash
brew tap skevetter/tap
brew install org-management
```

## Usage

Register an agent:

```bash
org-management register --name "build-eng" --type engineer --parent <manager-id>
```

Look up an agent:

```bash
org-management lookup --name "build-eng"
```

List children of an agent:

```bash
org-management children --id <agent-id>
```

Display the full org tree:

```bash
org-management tree --id <root-id>
```

Register and list artifacts:

```bash
org-management artifact register --agent <id> --type worktree --name "feature-x" --path ~/.paseo/worktrees/feature-x
org-management artifact list --agent <id>
```

All commands support `--json` for structured output and `--namespace` for multi-tenant isolation.

## MCP Server

Start the MCP server (stdio transport):

```bash
org-management serve
```

Add to your MCP client config (e.g. `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "org-management": {
      "command": "org-management",
      "args": ["serve"]
    }
  }
}
```

The server exposes 11 tools: `register_agent`, `deregister_agent`, `lookup_agent`, `list_children`, `list_ancestors`, `show_tree`, `register_artifact`, `list_artifacts`, `update_artifact`, `deregister_artifact`, and `search_agents`.

## License

MIT
