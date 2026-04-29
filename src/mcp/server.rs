use std::sync::Mutex;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ErrorData, tool, tool_router};

use crate::db::Database;

use super::tools::*;

pub struct OrgMcpServer {
    db: Mutex<Database>,
    default_namespace: Option<String>,
    default_actor: Option<String>,
}

impl OrgMcpServer {
    pub fn new(
        db: Database,
        default_namespace: Option<String>,
        default_actor: Option<String>,
    ) -> Self {
        Self {
            db: Mutex::new(db),
            default_namespace,
            default_actor,
        }
    }

    fn resolve_actor(&self, params_actor: Option<String>) -> Option<String> {
        params_actor.or_else(|| self.default_actor.clone())
    }

    fn resolve_namespace<'a>(&'a self, params_ns: &'a Option<String>) -> &'a str {
        params_ns
            .as_deref()
            .or(self.default_namespace.as_deref())
            .unwrap_or("default")
    }
}

#[tool_router(server_handler)]
impl OrgMcpServer {
    #[allow(dead_code)]
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder().enable_tools().build();
        ServerInfo::new(capabilities).with_server_info(Implementation::new(
            "org-management",
            env!("CARGO_PKG_VERSION"),
        ))
    }

    #[tool(description = "Register a new agent in the org hierarchy")]
    fn register_agent(
        &self,
        Parameters(params): Parameters<RegisterAgentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let actor = self.resolve_actor(params.actor);

        let db = self.db.lock().unwrap();
        let agent = db
            .register_agent(
                &params.name,
                &params.agent_type,
                params.parent_id.as_deref(),
                ns,
                params.metadata.as_deref(),
                actor.as_deref(),
            )
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&agent).unwrap(),
        )]))
    }

    #[tool(description = "Deregister (remove) an agent from the org hierarchy")]
    fn deregister_agent(
        &self,
        Parameters(params): Parameters<DeregisterAgentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let actor = self.resolve_actor(params.actor);

        let db = self.db.lock().unwrap();
        db.deregister_agent(&params.id, ns, actor.as_deref())
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&serde_json::json!({"deregistered": params.id})).unwrap(),
        )]))
    }

    #[tool(description = "Look up an agent by ID or name")]
    fn lookup_agent(
        &self,
        Parameters(params): Parameters<LookupAgentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if params.id.is_none() && params.name.is_none() {
            return Err(ErrorData::invalid_params(
                "Must provide either 'id' or 'name'",
                None,
            ));
        }

        let ns = self.resolve_namespace(&params.namespace);

        let db = self.db.lock().unwrap();
        let agent = db
            .lookup_agent(params.id.as_deref(), params.name.as_deref(), ns)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            .ok_or_else(|| ErrorData::invalid_params("Agent not found", None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&agent).unwrap(),
        )]))
    }

    #[tool(description = "List direct children of an agent")]
    fn list_children(
        &self,
        Parameters(params): Parameters<ListChildrenParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let limit = params.limit.unwrap_or(50);
        let offset = params.offset.unwrap_or(0);

        let db = self.db.lock().unwrap();
        let result = db
            .list_children(&params.id, ns, limit, offset)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&result).unwrap(),
        )]))
    }

    #[tool(description = "List ancestors of an agent from immediate parent to root")]
    fn list_ancestors(
        &self,
        Parameters(params): Parameters<ListAncestorsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);

        let db = self.db.lock().unwrap();
        let ancestors = db
            .list_ancestors(&params.id, ns)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&serde_json::json!({"ancestors": ancestors})).unwrap(),
        )]))
    }

    #[tool(description = "Get the full org tree or subtree from a given root")]
    fn get_tree(
        &self,
        Parameters(params): Parameters<GetTreeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);

        let db = self.db.lock().unwrap();
        let tree = db
            .get_tree(params.root_id.as_deref(), ns)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&serde_json::json!({"tree": tree})).unwrap(),
        )]))
    }

    #[tool(description = "Register a resource artifact owned by an agent")]
    fn register_artifact(
        &self,
        Parameters(params): Parameters<RegisterArtifactParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let actor = self.resolve_actor(params.actor);

        let db = self.db.lock().unwrap();
        let artifact = db
            .register_artifact(
                &params.agent_id,
                &params.artifact_type,
                &params.name,
                params.path.as_deref(),
                ns,
                actor.as_deref(),
            )
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&artifact).unwrap(),
        )]))
    }

    #[tool(description = "List artifacts owned by an agent")]
    fn list_artifacts(
        &self,
        Parameters(params): Parameters<ListArtifactsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let limit = params.limit.unwrap_or(50);
        let offset = params.offset.unwrap_or(0);

        let db = self.db.lock().unwrap();
        let result = db
            .list_artifacts(&params.agent_id, ns, limit, offset)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&result).unwrap(),
        )]))
    }

    #[tool(description = "Update an artifact's status or last_seen_at timestamp")]
    fn update_artifact(
        &self,
        Parameters(params): Parameters<UpdateArtifactParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let actor = self.resolve_actor(params.actor);

        let status = params.status.as_deref().unwrap_or("active");

        let db = self.db.lock().unwrap();
        let artifact = db
            .update_artifact(&params.id, status, ns, actor.as_deref())
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
            .ok_or_else(|| ErrorData::invalid_params("Artifact not found", None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&artifact).unwrap(),
        )]))
    }

    #[tool(description = "Deregister (remove) an artifact")]
    fn deregister_artifact(
        &self,
        Parameters(params): Parameters<DeregisterArtifactParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let actor = self.resolve_actor(params.actor);

        let db = self.db.lock().unwrap();
        db.deregister_artifact(&params.id, ns, actor.as_deref())
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&serde_json::json!({"deregistered": params.id})).unwrap(),
        )]))
    }

    #[tool(description = "Search the agent directory using FTS5 full-text search")]
    fn search_directory(
        &self,
        Parameters(params): Parameters<SearchDirectoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let ns = self.resolve_namespace(&params.namespace);
        let limit = params.limit.unwrap_or(20);

        let db = self.db.lock().unwrap();
        let result = db
            .search_agents(&params.query, ns, limit)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(
                &serde_json::json!({"results": result.agents, "total": result.total}),
            )
            .unwrap(),
        )]))
    }
}
