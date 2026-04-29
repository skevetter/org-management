# org-management — Design Document

## 1. Interface Contract

org-management is a combined CLI + MCP server for tracking agent hierarchy, relationships, and resource artifacts within multi-agent systems.

**Binary:** `org-management`

**Dual interface:**
- **CLI** — Human and script-friendly commands with `--json` structured output
- **MCP Server** — stdio transport via `org-management serve`, exposes 11 tools for agent-to-agent coordination

**Core entities:**
- **Agent** — A registered participant (engineer, manager, director, etc.) with a hierarchical position
- **Relationship** — Parent-child edge in the org adjacency list
- **Artifact** — A resource (worktree, room, schedule, branch) owned by an agent

**Data path:** `~/.local/share/org-management/org.db` (XDG_DATA_HOME compliant)

**Global flags:**
| Flag | Description |
|------|-------------|
| `--db <path>` | Override database path |
| `--json` | Emit structured JSON output |
| `-n, --namespace <ns>` | Scope operations to a namespace (default: "default") |

## 2. SQLite Schema

### 2.1 Agents Table

```sql
CREATE TABLE agents (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    agent_type  TEXT NOT NULL,          -- engineer, manager, director, senior-manager
    parent_agent_id TEXT,               -- NULL for root agents
    namespace   TEXT NOT NULL DEFAULT 'default',
    status      TEXT NOT NULL DEFAULT 'active',  -- active, inactive, archived
    metadata_json TEXT,                 -- arbitrary JSON blob
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(name, namespace),
    FOREIGN KEY (parent_agent_id) REFERENCES agents(id) ON DELETE SET NULL
);
```

### 2.2 Relationships Table

```sql
CREATE TABLE relationships (
    parent_id         TEXT NOT NULL,
    child_id          TEXT NOT NULL,
    relationship_type TEXT NOT NULL DEFAULT 'reports_to',  -- reports_to, owns, delegates_to
    namespace         TEXT NOT NULL DEFAULT 'default',
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (parent_id, child_id, namespace),
    FOREIGN KEY (parent_id) REFERENCES agents(id) ON DELETE CASCADE,
    FOREIGN KEY (child_id) REFERENCES agents(id) ON DELETE CASCADE
);
```

### 2.3 Artifacts Table

```sql
CREATE TABLE artifacts (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT NOT NULL,
    artifact_type TEXT NOT NULL,         -- worktree, room, schedule, branch
    name          TEXT NOT NULL,
    path          TEXT,                  -- filesystem path or URL (optional)
    status        TEXT NOT NULL DEFAULT 'active',  -- active, archived, deleted
    namespace     TEXT NOT NULL DEFAULT 'default',
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    last_seen_at  TEXT,
    UNIQUE(agent_id, artifact_type, name, namespace),
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);
```

### 2.4 FTS5 Virtual Table

```sql
CREATE VIRTUAL TABLE agents_fts USING fts5(
    name,
    agent_type,
    namespace,
    metadata_json,
    content='agents',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER agents_ai AFTER INSERT ON agents BEGIN
    INSERT INTO agents_fts(rowid, name, agent_type, namespace, metadata_json)
    VALUES (new.rowid, new.name, new.agent_type, new.namespace, new.metadata_json);
END;

CREATE TRIGGER agents_ad AFTER DELETE ON agents BEGIN
    INSERT INTO agents_fts(agents_fts, rowid, name, agent_type, namespace, metadata_json)
    VALUES ('delete', old.rowid, old.name, old.agent_type, old.namespace, old.metadata_json);
END;

CREATE TRIGGER agents_au AFTER UPDATE ON agents BEGIN
    INSERT INTO agents_fts(agents_fts, rowid, name, agent_type, namespace, metadata_json)
    VALUES ('delete', old.rowid, old.name, old.agent_type, old.namespace, old.metadata_json);
    INSERT INTO agents_fts(rowid, name, agent_type, namespace, metadata_json)
    VALUES (new.rowid, new.name, new.agent_type, new.namespace, new.metadata_json);
END;
```

### 2.5 Indexes

```sql
CREATE INDEX idx_agents_namespace ON agents(namespace);
CREATE INDEX idx_agents_parent ON agents(parent_agent_id);
CREATE INDEX idx_agents_status ON agents(namespace, status);

CREATE INDEX idx_relationships_parent ON relationships(parent_id, namespace);
CREATE INDEX idx_relationships_child ON relationships(child_id, namespace);

CREATE INDEX idx_artifacts_agent ON artifacts(agent_id);
CREATE INDEX idx_artifacts_namespace ON artifacts(namespace);
CREATE INDEX idx_artifacts_type ON artifacts(agent_id, artifact_type, namespace);
```

### 2.6 Pragmas (applied on connection)

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
```

## 3. CLI Command Reference

### 3.1 `register` — Register an agent

```
org-management register --name <name> --type <type> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--name <name>` | yes | Agent display name |
| `--type <type>` | yes | Role: engineer, manager, director, senior-manager |
| `--parent <id>` | no | Parent agent ID (creates relationship) |
| `--namespace <ns>` | no | Namespace scope (default: "default") |
| `--metadata <json>` | no | Arbitrary JSON metadata |
| `--actor <id>` | no | Who performed this action (default: $PASEO_AGENT_ID) |
| `--json` | no | Structured JSON output |

**Behavior:** UPSERT — if agent with same name+namespace exists, updates fields.

### 3.2 `deregister` — Remove an agent

```
org-management deregister --id <id> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Agent ID (supports short prefix) |
| `--namespace <ns>` | no | Namespace scope |
| `--actor <id>` | no | Who performed this action |
| `--json` | no | Structured JSON output |

**Behavior:** Sets status to "archived". CASCADE deletes relationships and artifacts.

### 3.3 `lookup` — Find an agent by ID or name

```
org-management lookup [--id <id> | --name <name>] [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | one of id/name | Agent ID (supports short prefix) |
| `--name <name>` | one of id/name | Agent name |
| `--namespace <ns>` | no | Namespace scope |
| `--json` | no | Structured JSON output |

### 3.4 `children` — List direct children

```
org-management children --id <id> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Parent agent ID |
| `--namespace <ns>` | no | Namespace scope |
| `--limit <n>` | no | Max results (default: 50) |
| `--offset <n>` | no | Pagination offset (default: 0) |
| `--json` | no | Structured JSON output |

### 3.5 `ancestors` — List ancestors to root

```
org-management ancestors --id <id> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Agent ID |
| `--namespace <ns>` | no | Namespace scope |
| `--json` | no | Structured JSON output |

**Returns:** Ordered list from immediate parent to root.

### 3.6 `tree` — Display full subtree

```
org-management tree [--root <id>] [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--root <id>` | no | Root agent ID (default: all roots in namespace) |
| `--namespace <ns>` | no | Namespace scope |
| `--json` | no | Structured JSON output |

**Text output:** Indented tree with agent names and types.
**JSON output:** Nested object with `children` arrays.

### 3.7 `artifact register` — Register a resource artifact

```
org-management artifact register --agent-id <id> --type <type> --name <name> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--agent-id <id>` | yes | Owning agent ID |
| `--type <type>` | yes | worktree, room, schedule, branch |
| `--name <name>` | yes | Artifact name/identifier |
| `--path <path>` | no | Filesystem path or URL |
| `--namespace <ns>` | no | Namespace scope |
| `--actor <id>` | no | Who performed this action |
| `--json` | no | Structured JSON output |

**Behavior:** UPSERT on (agent_id, artifact_type, name, namespace).

### 3.8 `artifact list` — List artifacts for an agent

```
org-management artifact list --agent-id <id> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--agent-id <id>` | yes | Agent ID |
| `--namespace <ns>` | no | Namespace scope |
| `--limit <n>` | no | Max results (default: 50) |
| `--offset <n>` | no | Pagination offset (default: 0) |
| `--json` | no | Structured JSON output |

### 3.9 `artifact update` — Update artifact status

```
org-management artifact update --id <id> --status <status> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Artifact ID |
| `--status <status>` | yes | New status: active, archived, deleted |
| `--namespace <ns>` | no | Namespace scope |
| `--actor <id>` | no | Who performed this action |
| `--json` | no | Structured JSON output |

### 3.10 `artifact deregister` — Remove an artifact

```
org-management artifact deregister --id <id> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Artifact ID |
| `--namespace <ns>` | no | Namespace scope |
| `--actor <id>` | no | Who performed this action |
| `--json` | no | Structured JSON output |

### 3.11 `serve` — Start MCP server

```
org-management serve [--transport stdio] [--namespace <ns>]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--transport <t>` | no | Transport type (default: "stdio", only stdio supported) |
| `--namespace <ns>` | no | Default namespace for MCP operations |

Starts the MCP server on stdin/stdout using rmcp 1.5 SDK.

## 4. MCP Tools

All tools accept JSON parameters and return JSON responses. Namespace defaults to "default" when omitted.

| Tool | Params | Returns |
|------|--------|---------|
| `register_agent` | name\*, agent_type\*, parent_id, namespace, metadata, actor | `Agent` |
| `deregister_agent` | id\*, namespace, actor | `{ deregistered: id }` |
| `lookup_agent` | id, name, namespace (one of id/name required) | `Agent` |
| `list_children` | id\*, namespace, limit (50), offset (0) | `{ agents: [Agent], total }` |
| `list_ancestors` | id\*, namespace | `{ ancestors: [Agent] }` |
| `get_tree` | root_id, namespace | `{ tree: [TreeNode] }` |
| `register_artifact` | agent_id\*, artifact_type\*, name\*, path, namespace, actor | `Artifact` |
| `list_artifacts` | agent_id\*, namespace, limit (50), offset (0) | `{ artifacts: [Artifact], total }` |
| `update_artifact` | id\*, status, last_seen_at, namespace, actor | `Artifact` |
| `deregister_artifact` | id\*, namespace, actor | `{ deregistered: id }` |
| `search_directory` | query\*, namespace, limit (20) | `{ results: [Agent], total }` |

\* = required. Defaults shown in parentheses.

### Return Types

**Agent:**
```json
{
  "id": "uuid", "name": "string", "agent_type": "string",
  "parent_agent_id": "string|null", "namespace": "string",
  "status": "string", "metadata_json": "string|null",
  "created_at": "ISO8601", "updated_at": "ISO8601"
}
```

**Artifact:**
```json
{
  "id": "uuid", "agent_id": "string", "artifact_type": "string",
  "name": "string", "path": "string|null", "status": "string",
  "namespace": "string", "created_at": "ISO8601",
  "updated_at": "ISO8601", "last_seen_at": "ISO8601|null"
}
```

**TreeNode:** `{ agent: Agent, children: [TreeNode] }`

`search_directory` supports FTS5 syntax: prefix matching (`*`), phrase matching (`"..."`).

## 5. Architecture Decisions

| Decision | Detail |
|----------|--------|
| **SQLite + WAL** | Single-file embedded DB. WAL mode for concurrent reads without blocking. `busy_timeout = 5000` avoids SQLITE_BUSY. |
| **XDG path** | `~/.local/share/org-management/org.db`. Respects `$XDG_DATA_HOME`. |
| **Namespace scoping (v0.1.0)** | Every entity includes `namespace`. UNIQUE constraints include namespace for multi-org isolation. Default: `"default"`. |
| **Actor tracking** | Every mutation accepts `--actor`. Auto-detected from `$PASEO_AGENT_ID` env var when omitted. |
| **Output modes** | Text (default, human-readable) and JSON (`--json`, matches MCP return types). |
| **Pagination** | `--limit` (default: 50) / `--offset` (default: 0) on all list ops. Returns include `total` count. |
| **Idempotent registration** | UPSERT on UNIQUE constraints. Safe to retry without error. |
| **FTS5 search** | Indexes agent name, type, namespace, metadata. Synced via triggers. Prefix/phrase/boolean queries. |
| **Cascading cleanup** | `ON DELETE CASCADE` on relationships and artifacts FKs. Deregister agent → removes all owned resources. |
| **MCP transport** | rmcp 1.5 SDK, stdio transport. `org-management serve`. Single-threaded tokio (current_thread). |

### 5.10 Rust Crate Structure

```
org-management/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI parsing (clap) and dispatch
│   ├── lib.rs           # Re-exports
│   ├── db.rs            # Database layer (SQLite operations)
│   ├── models.rs        # Structs: Agent, Relationship, Artifact
│   └── mcp/
│       ├── mod.rs       # Module declaration
│       ├── server.rs    # MCP server setup and tool dispatch
│       └── tools.rs     # Tool parameter structs (schemars)
└── tests/
    └── integration.rs   # assert_cmd + tempfile integration tests
```

### 5.11 Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
rmcp = { version = "1.5", features = ["server", "transport-io"] }
tokio = { version = "1", features = ["rt", "macros", "io-std"] }
schemars = "1"

[dev-dependencies]
assert_cmd = "2"
tempfile = "3"
predicates = "3"

## v0.2.0

### Schema Changes (v1 → v2 Migration)

The `agents` table gains two new nullable columns. On `db.rs::open()`, a migration function (`migrate_v1_to_v2`) runs `ALTER TABLE agents ADD COLUMN` for each missing column — making the upgrade non-destructive for existing databases.

```sql
-- Added to agents table
room          TEXT,                  -- chat room slug associated with agent
last_seen_at  TEXT                   -- ISO8601 timestamp of last heartbeat
```

**Migration logic:** `migrate_v1_to_v2` queries `pragma_table_info('agents')` and issues `ALTER TABLE ... ADD COLUMN` only when the column is absent. Safe to run on any schema version.

### New CLI Commands

#### `update-status` — Update agent status

```
org-management update-status --id <id> --status <status> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Agent ID (supports short prefix) |
| `--status <status>` | yes | New status: running, idle, blocked, done, active, inactive, archived |
| `--namespace <ns>` | no | Namespace scope |
| `--actor <id>` | no | Who performed this action |
| `--json` | no | Structured JSON output |

**Behavior:** Sets `status` and `updated_at`. Emits updated `Agent` record.

#### `list` — Flat agent list with filters

```
org-management list [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--status <s>` | no | Filter by status |
| `--parent <name>` | no | Filter by parent agent name |
| `--role <role>` | no | Filter by agent_type |
| `--namespace <ns>` | no | Namespace scope |
| `--limit <n>` | no | Max results (default: 50) |
| `--offset <n>` | no | Pagination offset (default: 0) |
| `--json` | no | Structured JSON output |

**Behavior:** Returns all agents in namespace matching optional filters. Each row includes id, name, type, status, parent name, room, and last_seen_at.

#### `bulk-deregister` — Remove all agents in a namespace

```
org-management bulk-deregister --org-id <org-id> [--cascade] [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--org-id <org-id>` | yes | Namespace to clear |
| `--cascade` | no | Also deregister agents that have children |
| `--json` | no | Structured JSON output |

**Behavior:** Archives all agents in the given namespace. Without `--cascade`, rejects if any agent has children. With `--cascade`, removes entire subtrees.

#### `heartbeat` — Update agent last_seen_at

```
org-management heartbeat --id <id> [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--id <id>` | yes | Agent ID (supports short prefix) |
| `--namespace <ns>` | no | Namespace scope |
| `--json` | no | Structured JSON output |

**Behavior:** Sets `last_seen_at` to current UTC timestamp. Used by agents to signal liveness.

#### `stale` — List running agents without recent heartbeat

```
org-management stale [--threshold <minutes>] [options]
```

| Flag | Required | Description |
|------|----------|-------------|
| `--threshold <minutes>` | no | Minutes since last heartbeat (default: 30) |
| `--namespace <ns>` | no | Namespace scope |
| `--json` | no | Structured JSON output |

**Behavior:** Returns agents with `status = 'running'` and `last_seen_at` older than threshold (or NULL). Used for detecting orphaned/stalled agents.

### Modified CLI Commands

#### `register` — Changes

Added `--status` and `--room` flags:

| Flag | Required | Description |
|------|----------|-------------|
| `--status <status>` | no | Initial status (default: active) |
| `--room <slug>` | no | Chat room slug associated with agent |

**Output change:** Now emits `CREATED` or `UPDATED` prefix on text output and adds `"action": "created" | "updated"` to JSON output. Fixes the silent-upsert problem where callers could not tell whether a new agent was created or an existing one was updated.

#### `deregister` — Changes

| Flag | Required | Description |
|------|----------|-------------|
| `--cascade` | no | Also remove all descendant agents (full subtree) |

**Behavior change (BUG-1 fix):** Without `--cascade`, `deregister` now rejects with exit code 1 if the target agent has any children. Before this fix, deregistering a parent silently left children with a dangling parent reference. With `--cascade`, the full subtree is recursively deregistered.

### New MCP Tools

| Tool | Params | Returns |
|------|--------|---------|
| `update_agent_status` | agent_id\*, status\*, namespace | `Agent` |
| `list_agents` | namespace, status, parent, role, limit (50), offset (0) | `{ agents: [Agent], total }` |
| `bulk_deregister_agents` | org_id\*, cascade (false) | `{ deregistered: [id] }` |
| `agent_heartbeat` | agent_id\*, namespace | `Agent` |
| `list_stale_agents` | threshold_minutes (30), namespace | `{ agents: [Agent], total }` |

\* = required. Defaults shown in parentheses.

### Modified MCP Tools

| Tool | Change |
|------|--------|
| `register_agent` | Added `status`, `room` params. Response includes `"action": "created" \| "updated"`. |
| `deregister_agent` | Added `cascade` param (default false). Returns error if agent has children and cascade is false. |
| `list_children` | Added optional `status` filter param. |

### Bug Fixes

| Bug | Description | Fix |
|-----|-------------|-----|
| **BUG-1** | `deregister` silently orphaned child agents when a parent was removed | `deregister` now exits 1 with an error if children exist; `--cascade` flag removes the full subtree cleanly |
| **BUG-2** | `register` silently upserted without indicating whether the agent was new or existing | `register` now outputs `CREATED` or `UPDATED` in text mode and `"action"` field in JSON mode |
```
