use chrono::Utc;
use rusqlite::{Connection, Result, params};
use uuid::Uuid;

use crate::models::{
    Agent, AgentListResult, Artifact, ArtifactListResult, ChildInfo, DeregisterResult,
    RegisterResult, TreeNode,
};

pub struct Database {
    conn: Connection,
}

fn migrate_v1_to_v2(conn: &Connection) -> Result<()> {
    let mut has_room = false;
    let mut has_last_seen_at = false;

    let mut stmt = conn.prepare("PRAGMA table_info(agents)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for col in rows {
        match col?.as_str() {
            "room" => has_room = true,
            "last_seen_at" => has_last_seen_at = true,
            _ => {}
        }
    }

    if !has_room {
        conn.execute_batch("ALTER TABLE agents ADD COLUMN room TEXT")?;
    }
    if !has_last_seen_at {
        conn.execute_batch("ALTER TABLE agents ADD COLUMN last_seen_at TEXT")?;
    }

    Ok(())
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agents (
                 id              TEXT PRIMARY KEY,
                 name            TEXT NOT NULL,
                 agent_type      TEXT NOT NULL,
                 parent_agent_id TEXT,
                 namespace       TEXT NOT NULL DEFAULT 'default',
                 status          TEXT NOT NULL DEFAULT 'active',
                 room            TEXT,
                 last_seen_at    TEXT,
                 metadata_json   TEXT,
                 created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                 updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                 UNIQUE(name, namespace),
                 FOREIGN KEY (parent_agent_id) REFERENCES agents(id) ON DELETE SET NULL
             );

             CREATE TABLE IF NOT EXISTS relationships (
                 parent_id         TEXT NOT NULL,
                 child_id          TEXT NOT NULL,
                 relationship_type TEXT NOT NULL DEFAULT 'reports_to',
                 namespace         TEXT NOT NULL DEFAULT 'default',
                 created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                 PRIMARY KEY (parent_id, child_id, namespace),
                 FOREIGN KEY (parent_id) REFERENCES agents(id) ON DELETE CASCADE,
                 FOREIGN KEY (child_id) REFERENCES agents(id) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS artifacts (
                 id            TEXT PRIMARY KEY,
                 agent_id      TEXT NOT NULL,
                 artifact_type TEXT NOT NULL,
                 name          TEXT NOT NULL,
                 path          TEXT,
                 status        TEXT NOT NULL DEFAULT 'active',
                 namespace     TEXT NOT NULL DEFAULT 'default',
                 created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                 updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                 last_seen_at  TEXT,
                 UNIQUE(agent_id, artifact_type, name, namespace),
                 FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
             );

             CREATE INDEX IF NOT EXISTS idx_agents_namespace ON agents(namespace);
             CREATE INDEX IF NOT EXISTS idx_agents_parent ON agents(parent_agent_id);
             CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(namespace, status);

             CREATE INDEX IF NOT EXISTS idx_relationships_parent ON relationships(parent_id, namespace);
             CREATE INDEX IF NOT EXISTS idx_relationships_child ON relationships(child_id, namespace);

             CREATE INDEX IF NOT EXISTS idx_artifacts_agent ON artifacts(agent_id);
             CREATE INDEX IF NOT EXISTS idx_artifacts_namespace ON artifacts(namespace);
             CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts(agent_id, artifact_type, namespace);",
        )?;

        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS agents_fts USING fts5(
                 name,
                 agent_type,
                 namespace,
                 metadata_json,
                 content='agents',
                 content_rowid='rowid'
             );

             CREATE TRIGGER IF NOT EXISTS agents_ai AFTER INSERT ON agents BEGIN
                 INSERT INTO agents_fts(rowid, name, agent_type, namespace, metadata_json)
                 VALUES (new.rowid, new.name, new.agent_type, new.namespace, new.metadata_json);
             END;

             CREATE TRIGGER IF NOT EXISTS agents_ad AFTER DELETE ON agents BEGIN
                 INSERT INTO agents_fts(agents_fts, rowid, name, agent_type, namespace, metadata_json)
                 VALUES ('delete', old.rowid, old.name, old.agent_type, old.namespace, old.metadata_json);
             END;

             CREATE TRIGGER IF NOT EXISTS agents_au AFTER UPDATE ON agents BEGIN
                 INSERT INTO agents_fts(agents_fts, rowid, name, agent_type, namespace, metadata_json)
                 VALUES ('delete', old.rowid, old.name, old.agent_type, old.namespace, old.metadata_json);
                 INSERT INTO agents_fts(rowid, name, agent_type, namespace, metadata_json)
                 VALUES (new.rowid, new.name, new.agent_type, new.namespace, new.metadata_json);
             END;",
        )?;

        migrate_v1_to_v2(&conn)?;

        Ok(Self { conn })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_agent(
        &self,
        name: &str,
        agent_type: &str,
        parent_id: Option<&str>,
        namespace: &str,
        metadata: Option<&str>,
        _actor: Option<&str>,
        status: Option<&str>,
        room: Option<&str>,
    ) -> Result<RegisterResult> {
        let already_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM agents WHERE name = ?1 AND namespace = ?2",
            params![name, namespace],
            |row| row.get::<_, i64>(0),
        )? > 0;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let status = status.unwrap_or("running");

        self.conn.execute(
            "INSERT INTO agents (id, name, agent_type, parent_agent_id, namespace, status, room, metadata_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(name, namespace) DO UPDATE SET
                 agent_type = excluded.agent_type,
                 parent_agent_id = excluded.parent_agent_id,
                 metadata_json = excluded.metadata_json,
                 status = excluded.status,
                 room = excluded.room,
                 updated_at = excluded.updated_at",
            params![id, name, agent_type, parent_id, namespace, status, room, metadata, now, now],
        )?;

        if let Some(pid) = parent_id {
            let actual_id = self.conn.query_row(
                "SELECT id FROM agents WHERE name = ?1 AND namespace = ?2",
                params![name, namespace],
                |row| row.get::<_, String>(0),
            )?;
            self.conn.execute(
                "INSERT OR REPLACE INTO relationships (parent_id, child_id, relationship_type, namespace)
                 VALUES (?1, ?2, 'reports_to', ?3)",
                params![pid, actual_id, namespace],
            )?;
        }

        let agent = self.get_agent_by_name(name, namespace)?;
        Ok(RegisterResult {
            agent,
            created: !already_exists,
        })
    }

    pub fn deregister_agent(
        &self,
        id: &str,
        namespace: &str,
        _actor: Option<&str>,
        cascade: bool,
    ) -> Result<DeregisterResult> {
        let full_id = self.resolve_agent_id(id, namespace)?;

        let children = self.get_children_info(&full_id, namespace)?;

        if !children.is_empty() && !cascade {
            return Ok(DeregisterResult::HasChildren(children));
        }

        if cascade && !children.is_empty() {
            let count = self.cascade_delete(&full_id, namespace)?;
            return Ok(DeregisterResult::Cascaded(count));
        }

        self.conn.execute(
            "DELETE FROM agents WHERE id = ?1 AND namespace = ?2",
            params![full_id, namespace],
        )?;
        Ok(DeregisterResult::Deleted)
    }

    fn get_children_info(&self, parent_id: &str, namespace: &str) -> Result<Vec<ChildInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name FROM relationships r
             JOIN agents a ON a.id = r.child_id
             WHERE r.parent_id = ?1 AND r.namespace = ?2",
        )?;
        let children = stmt
            .query_map(params![parent_id, namespace], |row| {
                Ok(ChildInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(children)
    }

    fn collect_descendants(
        &self,
        id: &str,
        namespace: &str,
        result: &mut Vec<String>,
    ) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT child_id FROM relationships WHERE parent_id = ?1 AND namespace = ?2",
        )?;
        let child_ids: Vec<String> = stmt
            .query_map(params![id, namespace], |row| row.get(0))?
            .collect::<Result<Vec<_>>>()?;

        for child_id in child_ids {
            self.collect_descendants(&child_id, namespace, result)?;
            result.push(child_id);
        }
        Ok(())
    }

    fn cascade_delete(&self, id: &str, namespace: &str) -> Result<i64> {
        let mut to_delete = Vec::new();
        self.collect_descendants(id, namespace, &mut to_delete)?;
        to_delete.push(id.to_string());

        let count = to_delete.len() as i64;
        for agent_id in &to_delete {
            self.conn.execute(
                "DELETE FROM artifacts WHERE agent_id = ?1 AND namespace = ?2",
                params![agent_id, namespace],
            )?;
            self.conn.execute(
                "DELETE FROM agents WHERE id = ?1 AND namespace = ?2",
                params![agent_id, namespace],
            )?;
        }
        Ok(count)
    }

    pub fn update_agent_status(&self, id: &str, status: &str, namespace: &str) -> Result<Agent> {
        let full_id = self.resolve_agent_id(id, namespace)?;
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.conn.execute(
            "UPDATE agents SET status = ?1, last_seen_at = ?2, updated_at = ?2 WHERE id = ?3 AND namespace = ?4",
            params![status, now, full_id, namespace],
        )?;

        self.get_agent_by_id(&full_id)
    }

    pub fn lookup_agent(
        &self,
        id: Option<&str>,
        name: Option<&str>,
        namespace: &str,
    ) -> Result<Option<Agent>> {
        if let Some(id_val) = id {
            let full_id = match self.resolve_agent_id(id_val, namespace) {
                Ok(fid) => fid,
                Err(_) => return Ok(None),
            };
            let mut stmt = self.conn.prepare(
                "SELECT id, name, agent_type, parent_agent_id, namespace, status, room, last_seen_at, metadata_json, created_at, updated_at
                 FROM agents WHERE id = ?1 AND namespace = ?2",
            )?;
            let result = stmt.query_row(params![full_id, namespace], |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            });
            match result {
                Ok(agent) => Ok(Some(agent)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        } else if let Some(name_val) = name {
            let mut stmt = self.conn.prepare(
                "SELECT id, name, agent_type, parent_agent_id, namespace, status, room, last_seen_at, metadata_json, created_at, updated_at
                 FROM agents WHERE name = ?1 AND namespace = ?2",
            )?;
            let result = stmt.query_row(params![name_val, namespace], |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            });
            match result {
                Ok(agent) => Ok(Some(agent)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }

    pub fn list_children(
        &self,
        parent_id: &str,
        namespace: &str,
        limit: i64,
        offset: i64,
        status_filter: Option<&str>,
    ) -> Result<AgentListResult> {
        let full_id = self.resolve_agent_id(parent_id, namespace)?;

        let (total, agents) = if let Some(status) = status_filter {
            let total: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM relationships r
                 JOIN agents a ON a.id = r.child_id
                 WHERE r.parent_id = ?1 AND r.namespace = ?2 AND a.status = ?3",
                params![full_id, namespace, status],
                |row| row.get(0),
            )?;

            let mut stmt = self.conn.prepare(
                "SELECT a.id, a.name, a.agent_type, a.parent_agent_id, a.namespace, a.status, a.room, a.last_seen_at, a.metadata_json, a.created_at, a.updated_at
                 FROM relationships r
                 JOIN agents a ON a.id = r.child_id
                 WHERE r.parent_id = ?1 AND r.namespace = ?2 AND a.status = ?3
                 ORDER BY a.name
                 LIMIT ?4 OFFSET ?5",
            )?;

            let agents = stmt
                .query_map(params![full_id, namespace, status, limit, offset], |row| {
                    Ok(Agent {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        agent_type: row.get(2)?,
                        parent_agent_id: row.get(3)?,
                        namespace: row.get(4)?,
                        status: row.get(5)?,
                        room: row.get(6)?,
                        last_seen_at: row.get(7)?,
                        metadata_json: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            (total, agents)
        } else {
            let total: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM relationships r
                 JOIN agents a ON a.id = r.child_id
                 WHERE r.parent_id = ?1 AND r.namespace = ?2",
                params![full_id, namespace],
                |row| row.get(0),
            )?;

            let mut stmt = self.conn.prepare(
                "SELECT a.id, a.name, a.agent_type, a.parent_agent_id, a.namespace, a.status, a.room, a.last_seen_at, a.metadata_json, a.created_at, a.updated_at
                 FROM relationships r
                 JOIN agents a ON a.id = r.child_id
                 WHERE r.parent_id = ?1 AND r.namespace = ?2
                 ORDER BY a.name
                 LIMIT ?3 OFFSET ?4",
            )?;

            let agents = stmt
                .query_map(params![full_id, namespace, limit, offset], |row| {
                    Ok(Agent {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        agent_type: row.get(2)?,
                        parent_agent_id: row.get(3)?,
                        namespace: row.get(4)?,
                        status: row.get(5)?,
                        room: row.get(6)?,
                        last_seen_at: row.get(7)?,
                        metadata_json: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            (total, agents)
        };

        Ok(AgentListResult { agents, total })
    }

    pub fn list_ancestors(&self, id: &str, namespace: &str) -> Result<Vec<Agent>> {
        let full_id = self.resolve_agent_id(id, namespace)?;
        let mut ancestors = Vec::new();
        let mut current_id = full_id;

        loop {
            let parent_id: Option<String> = self.conn.query_row(
                "SELECT parent_id FROM relationships WHERE child_id = ?1 AND namespace = ?2 LIMIT 1",
                params![current_id, namespace],
                |row| row.get(0),
            ).optional()?;

            match parent_id {
                Some(pid) => {
                    let agent = self.get_agent_by_id(&pid)?;
                    ancestors.push(agent);
                    current_id = pid;
                }
                None => break,
            }
        }

        Ok(ancestors)
    }

    pub fn get_tree(&self, root_id: Option<&str>, namespace: &str) -> Result<Vec<TreeNode>> {
        match root_id {
            Some(rid) => {
                let full_id = self.resolve_agent_id(rid, namespace)?;
                let agent = self.get_agent_by_id(&full_id)?;
                let node = self.build_tree_node(agent, namespace)?;
                Ok(vec![node])
            }
            None => {
                let mut stmt = self.conn.prepare(
                    "SELECT id, name, agent_type, parent_agent_id, namespace, status, room, last_seen_at, metadata_json, created_at, updated_at
                     FROM agents
                     WHERE namespace = ?1
                     AND id NOT IN (SELECT child_id FROM relationships WHERE namespace = ?1)
                     ORDER BY name",
                )?;
                let roots = stmt
                    .query_map(params![namespace], |row| {
                        Ok(Agent {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            agent_type: row.get(2)?,
                            parent_agent_id: row.get(3)?,
                            namespace: row.get(4)?,
                            status: row.get(5)?,
                            room: row.get(6)?,
                            last_seen_at: row.get(7)?,
                            metadata_json: row.get(8)?,
                            created_at: row.get(9)?,
                            updated_at: row.get(10)?,
                        })
                    })?
                    .collect::<Result<Vec<_>>>()?;

                let mut tree = Vec::new();
                for agent in roots {
                    tree.push(self.build_tree_node(agent, namespace)?);
                }
                Ok(tree)
            }
        }
    }

    pub fn register_artifact(
        &self,
        agent_id: &str,
        artifact_type: &str,
        name: &str,
        path: Option<&str>,
        namespace: &str,
        _actor: Option<&str>,
    ) -> Result<Artifact> {
        let full_agent_id = self.resolve_agent_id(agent_id, namespace)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.conn.execute(
            "INSERT INTO artifacts (id, agent_id, artifact_type, name, path, namespace, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(agent_id, artifact_type, name, namespace) DO UPDATE SET
                 path = excluded.path,
                 status = 'active',
                 updated_at = excluded.updated_at",
            params![id, full_agent_id, artifact_type, name, path, namespace, now, now],
        )?;

        self.get_artifact_by_unique(agent_id, artifact_type, name, namespace)
    }

    pub fn list_artifacts(
        &self,
        agent_id: &str,
        namespace: &str,
        limit: i64,
        offset: i64,
    ) -> Result<ArtifactListResult> {
        let full_id = self.resolve_agent_id(agent_id, namespace)?;

        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM artifacts WHERE agent_id = ?1 AND namespace = ?2",
            params![full_id, namespace],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, artifact_type, name, path, status, namespace, created_at, updated_at, last_seen_at
             FROM artifacts WHERE agent_id = ?1 AND namespace = ?2
             ORDER BY name
             LIMIT ?3 OFFSET ?4",
        )?;

        let artifacts = stmt
            .query_map(params![full_id, namespace, limit, offset], |row| {
                Ok(Artifact {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    artifact_type: row.get(2)?,
                    name: row.get(3)?,
                    path: row.get(4)?,
                    status: row.get(5)?,
                    namespace: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    last_seen_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(ArtifactListResult { artifacts, total })
    }

    pub fn update_artifact(
        &self,
        id: &str,
        status: &str,
        namespace: &str,
        _actor: Option<&str>,
    ) -> Result<Option<Artifact>> {
        let full_id = self.resolve_artifact_id(id, namespace)?;
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let updated = self.conn.execute(
            "UPDATE artifacts SET status = ?1, updated_at = ?2 WHERE id = ?3 AND namespace = ?4",
            params![status, now, full_id, namespace],
        )?;

        if updated == 0 {
            return Ok(None);
        }

        let artifact = self.get_artifact_by_id(&full_id)?;
        Ok(Some(artifact))
    }

    pub fn deregister_artifact(
        &self,
        id: &str,
        namespace: &str,
        _actor: Option<&str>,
    ) -> Result<()> {
        let full_id = self.resolve_artifact_id(id, namespace)?;
        self.conn.execute(
            "DELETE FROM artifacts WHERE id = ?1 AND namespace = ?2",
            params![full_id, namespace],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn search_agents(
        &self,
        query: &str,
        namespace: &str,
        limit: i64,
    ) -> Result<AgentListResult> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, a.agent_type, a.parent_agent_id, a.namespace, a.status, a.room, a.last_seen_at, a.metadata_json, a.created_at, a.updated_at
             FROM agents_fts f
             JOIN agents a ON a.rowid = f.rowid
             WHERE agents_fts MATCH ?1 AND a.namespace = ?2
             LIMIT ?3",
        )?;

        let agents = stmt
            .query_map(params![query, namespace, limit], |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        let total = agents.len() as i64;
        Ok(AgentListResult { agents, total })
    }

    pub fn list_agents(
        &self,
        namespace: &str,
        status_filter: Option<&str>,
        parent_name_filter: Option<&str>,
        role_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<AgentListResult> {
        let parent_id = if let Some(parent_name) = parent_name_filter {
            let id: String = self
                .conn
                .query_row(
                    "SELECT id FROM agents WHERE name = ?1 AND namespace = ?2",
                    params![parent_name, namespace],
                    |row| row.get(0),
                )
                .map_err(|_| rusqlite::Error::QueryReturnedNoRows)?;
            Some(id)
        } else {
            None
        };

        let mut conditions = vec!["a.namespace = ?1".to_string()];
        let mut count_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(namespace.to_string())];

        let use_join = parent_id.is_some();
        if let Some(ref pid) = parent_id {
            count_params.push(Box::new(pid.clone()));
            conditions.push(format!("r.parent_id = ?{}", count_params.len()));
        }
        if let Some(status) = status_filter {
            count_params.push(Box::new(status.to_string()));
            conditions.push(format!("a.status = ?{}", count_params.len()));
        }
        if let Some(role) = role_filter {
            count_params.push(Box::new(role.to_string()));
            conditions.push(format!("a.agent_type = ?{}", count_params.len()));
        }

        let join_clause = if use_join {
            "JOIN relationships r ON r.child_id = a.id AND r.namespace = a.namespace"
        } else {
            ""
        };

        let where_clause = conditions.join(" AND ");

        let count_sql = format!(
            "SELECT COUNT(*) FROM agents a {} WHERE {}",
            join_clause, where_clause
        );
        let total: i64 = self.conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(count_params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;

        let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(namespace.to_string())];
        if let Some(ref pid) = parent_id {
            query_params.push(Box::new(pid.clone()));
        }
        if let Some(status) = status_filter {
            query_params.push(Box::new(status.to_string()));
        }
        if let Some(role) = role_filter {
            query_params.push(Box::new(role.to_string()));
        }
        query_params.push(Box::new(limit));
        let limit_idx = query_params.len();
        query_params.push(Box::new(offset));
        let offset_idx = query_params.len();

        let select_sql = format!(
            "SELECT a.id, a.name, a.agent_type, a.parent_agent_id, a.namespace, a.status, a.room, a.last_seen_at, a.metadata_json, a.created_at, a.updated_at
             FROM agents a {} WHERE {} ORDER BY a.name LIMIT ?{} OFFSET ?{}",
            join_clause, where_clause, limit_idx, offset_idx
        );

        let mut stmt = self.conn.prepare(&select_sql)?;
        let agents = stmt
            .query_map(
                rusqlite::params_from_iter(query_params.iter().map(|p| p.as_ref())),
                |row| {
                    Ok(Agent {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        agent_type: row.get(2)?,
                        parent_agent_id: row.get(3)?,
                        namespace: row.get(4)?,
                        status: row.get(5)?,
                        room: row.get(6)?,
                        last_seen_at: row.get(7)?,
                        metadata_json: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>>>()?;

        Ok(AgentListResult { agents, total })
    }

    pub fn resolve_agent_id(&self, prefix: &str, namespace: &str) -> Result<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM agents WHERE id LIKE ?1 AND namespace = ?2")?;
        let pattern = format!("{prefix}%");
        let ids: Vec<String> = stmt
            .query_map(params![pattern, namespace], |row| row.get(0))?
            .collect::<Result<Vec<_>>>()?;

        match ids.len() {
            0 => Err(rusqlite::Error::QueryReturnedNoRows),
            1 => Ok(ids.into_iter().next().unwrap()),
            _ => {
                if ids.contains(&prefix.to_string()) {
                    Ok(prefix.to_string())
                } else {
                    Err(rusqlite::Error::QueryReturnedNoRows)
                }
            }
        }
    }

    fn resolve_artifact_id(&self, prefix: &str, namespace: &str) -> Result<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM artifacts WHERE id LIKE ?1 AND namespace = ?2")?;
        let pattern = format!("{prefix}%");
        let ids: Vec<String> = stmt
            .query_map(params![pattern, namespace], |row| row.get(0))?
            .collect::<Result<Vec<_>>>()?;

        match ids.len() {
            0 => Err(rusqlite::Error::QueryReturnedNoRows),
            1 => Ok(ids.into_iter().next().unwrap()),
            _ => {
                if ids.contains(&prefix.to_string()) {
                    Ok(prefix.to_string())
                } else {
                    Err(rusqlite::Error::QueryReturnedNoRows)
                }
            }
        }
    }

    fn get_agent_by_name(&self, name: &str, namespace: &str) -> Result<Agent> {
        self.conn.query_row(
            "SELECT id, name, agent_type, parent_agent_id, namespace, status, room, last_seen_at, metadata_json, created_at, updated_at
             FROM agents WHERE name = ?1 AND namespace = ?2",
            params![name, namespace],
            |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
    }

    fn get_agent_by_id(&self, id: &str) -> Result<Agent> {
        self.conn.query_row(
            "SELECT id, name, agent_type, parent_agent_id, namespace, status, room, last_seen_at, metadata_json, created_at, updated_at
             FROM agents WHERE id = ?1",
            params![id],
            |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
    }

    fn get_artifact_by_unique(
        &self,
        agent_id: &str,
        artifact_type: &str,
        name: &str,
        namespace: &str,
    ) -> Result<Artifact> {
        let full_agent_id = self.resolve_agent_id(agent_id, namespace)?;
        self.conn.query_row(
            "SELECT id, agent_id, artifact_type, name, path, status, namespace, created_at, updated_at, last_seen_at
             FROM artifacts WHERE agent_id = ?1 AND artifact_type = ?2 AND name = ?3 AND namespace = ?4",
            params![full_agent_id, artifact_type, name, namespace],
            |row| {
                Ok(Artifact {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    artifact_type: row.get(2)?,
                    name: row.get(3)?,
                    path: row.get(4)?,
                    status: row.get(5)?,
                    namespace: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    last_seen_at: row.get(9)?,
                })
            },
        )
    }

    fn get_artifact_by_id(&self, id: &str) -> Result<Artifact> {
        self.conn.query_row(
            "SELECT id, agent_id, artifact_type, name, path, status, namespace, created_at, updated_at, last_seen_at
             FROM artifacts WHERE id = ?1",
            params![id],
            |row| {
                Ok(Artifact {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    artifact_type: row.get(2)?,
                    name: row.get(3)?,
                    path: row.get(4)?,
                    status: row.get(5)?,
                    namespace: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    last_seen_at: row.get(9)?,
                })
            },
        )
    }

    fn build_tree_node(&self, agent: Agent, namespace: &str) -> Result<TreeNode> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.name, a.agent_type, a.parent_agent_id, a.namespace, a.status, a.room, a.last_seen_at, a.metadata_json, a.created_at, a.updated_at
             FROM relationships r
             JOIN agents a ON a.id = r.child_id
             WHERE r.parent_id = ?1 AND r.namespace = ?2
             ORDER BY a.name",
        )?;

        let child_agents = stmt
            .query_map(params![agent.id, namespace], |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        let mut children = Vec::new();
        for child in child_agents {
            children.push(self.build_tree_node(child, namespace)?);
        }

        Ok(TreeNode { agent, children })
    }

    pub fn bulk_deregister(&self, namespace: &str, cascade: bool) -> Result<(i64, i64)> {
        if cascade {
            let artifacts_count: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM artifacts WHERE namespace = ?1",
                params![namespace],
                |row| row.get(0),
            )?;
            self.conn.execute(
                "DELETE FROM artifacts WHERE namespace = ?1",
                params![namespace],
            )?;
            self.conn.execute(
                "DELETE FROM relationships WHERE namespace = ?1",
                params![namespace],
            )?;
            let agents_count: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM agents WHERE namespace = ?1",
                params![namespace],
                |row| row.get(0),
            )?;
            self.conn.execute(
                "DELETE FROM agents WHERE namespace = ?1",
                params![namespace],
            )?;
            Ok((agents_count, artifacts_count))
        } else {
            let agents_count = self.conn.execute(
                "DELETE FROM agents WHERE namespace = ?1 AND id NOT IN (SELECT parent_id FROM relationships WHERE namespace = ?1)",
                params![namespace],
            )? as i64;
            Ok((agents_count, 0))
        }
    }

    pub fn agent_heartbeat(&self, id: &str, namespace: &str) -> Result<Agent> {
        let full_id = self.resolve_agent_id(id, namespace)?;
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        self.conn.execute(
            "UPDATE agents SET last_seen_at = ?1, updated_at = ?1 WHERE id = ?2 AND namespace = ?3",
            params![now, full_id, namespace],
        )?;

        self.get_agent_by_id(&full_id)
    }

    pub fn list_stale_agents(
        &self,
        namespace: &str,
        threshold_minutes: i64,
    ) -> Result<AgentListResult> {
        let threshold_str = format!("-{threshold_minutes} minutes");

        let mut stmt = self.conn.prepare(
            "SELECT id, name, agent_type, parent_agent_id, namespace, status, room, last_seen_at, metadata_json, created_at, updated_at
             FROM agents
             WHERE namespace = ?1
               AND status = 'running'
               AND (last_seen_at IS NULL OR last_seen_at < strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?2))
             ORDER BY name",
        )?;

        let agents = stmt
            .query_map(params![namespace, threshold_str], |row| {
                Ok(Agent {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_type: row.get(2)?,
                    parent_agent_id: row.get(3)?,
                    namespace: row.get(4)?,
                    status: row.get(5)?,
                    room: row.get(6)?,
                    last_seen_at: row.get(7)?,
                    metadata_json: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        let total = agents.len() as i64;
        Ok(AgentListResult { agents, total })
    }
}

trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> OptionalExt<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
