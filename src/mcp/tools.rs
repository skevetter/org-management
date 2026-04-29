#![allow(dead_code)]

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RegisterAgentParams {
    pub name: String,
    pub agent_type: String,
    pub parent_id: Option<String>,
    pub namespace: Option<String>,
    pub metadata: Option<String>,
    pub actor: Option<String>,
    pub status: Option<String>,
    pub room: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeregisterAgentParams {
    pub id: String,
    pub namespace: Option<String>,
    pub actor: Option<String>,
    pub cascade: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LookupAgentParams {
    pub id: Option<String>,
    pub name: Option<String>,
    pub namespace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListChildrenParams {
    pub id: String,
    pub namespace: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateAgentStatusParams {
    pub agent_id: String,
    pub status: String,
    pub namespace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListAncestorsParams {
    pub id: String,
    pub namespace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetTreeParams {
    pub root_id: Option<String>,
    pub namespace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RegisterArtifactParams {
    pub agent_id: String,
    pub artifact_type: String,
    pub name: String,
    pub path: Option<String>,
    pub namespace: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListArtifactsParams {
    pub agent_id: String,
    pub namespace: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateArtifactParams {
    pub id: String,
    pub status: Option<String>,
    pub last_seen_at: Option<String>,
    pub namespace: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeregisterArtifactParams {
    pub id: String,
    pub namespace: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListAgentsParams {
    pub namespace: Option<String>,
    pub status: Option<String>,
    pub parent: Option<String>,
    pub role: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchDirectoryParams {
    pub query: String,
    pub namespace: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BulkDeregisterParams {
    pub org_id: String,
    pub cascade: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AgentHeartbeatParams {
    pub agent_id: String,
    pub namespace: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListStaleAgentsParams {
    pub threshold_minutes: Option<i64>,
    pub namespace: Option<String>,
}
