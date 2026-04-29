use std::fmt;
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    Engineer,
    Manager,
    Director,
    SeniorManager,
}

impl fmt::Display for AgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engineer => write!(f, "engineer"),
            Self::Manager => write!(f, "manager"),
            Self::Director => write!(f, "director"),
            Self::SeniorManager => write!(f, "senior-manager"),
        }
    }
}

impl FromStr for AgentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "engineer" => Ok(Self::Engineer),
            "manager" => Ok(Self::Manager),
            "director" => Ok(Self::Director),
            "senior-manager" | "senior_manager" | "seniormanager" => Ok(Self::SeniorManager),
            _ => Err(format!("unknown agent type: {s}")),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Inactive,
    Archived,
    Running,
    Idle,
    Blocked,
    Done,
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Archived => write!(f, "archived"),
            Self::Running => write!(f, "running"),
            Self::Idle => write!(f, "idle"),
            Self::Blocked => write!(f, "blocked"),
            Self::Done => write!(f, "done"),
        }
    }
}

impl FromStr for AgentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "archived" => Ok(Self::Archived),
            "running" => Ok(Self::Running),
            "idle" => Ok(Self::Idle),
            "blocked" => Ok(Self::Blocked),
            "done" => Ok(Self::Done),
            _ => Err(format!("unknown agent status: {s}")),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactType {
    Worktree,
    Room,
    Schedule,
    Branch,
}

impl fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Worktree => write!(f, "worktree"),
            Self::Room => write!(f, "room"),
            Self::Schedule => write!(f, "schedule"),
            Self::Branch => write!(f, "branch"),
        }
    }
}

impl FromStr for ArtifactType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "worktree" => Ok(Self::Worktree),
            "room" => Ok(Self::Room),
            "schedule" => Ok(Self::Schedule),
            "branch" => Ok(Self::Branch),
            _ => Err(format!("unknown artifact type: {s}")),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactStatus {
    Active,
    Archived,
    Deleted,
}

impl fmt::Display for ArtifactStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Archived => write!(f, "archived"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

impl FromStr for ArtifactStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(format!("unknown artifact status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub agent_type: String,
    pub parent_agent_id: Option<String>,
    pub namespace: String,
    pub status: String,
    pub room: Option<String>,
    pub last_seen_at: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl fmt::Display for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ID:        {}", self.id)?;
        writeln!(f, "Name:      {}", self.name)?;
        writeln!(f, "Type:      {}", self.agent_type)?;
        if let Some(parent) = &self.parent_agent_id {
            writeln!(f, "Parent:    {parent}")?;
        }
        writeln!(f, "Namespace: {}", self.namespace)?;
        writeln!(f, "Status:    {}", self.status)?;
        if let Some(room) = &self.room {
            writeln!(f, "Room:      {room}")?;
        }
        if let Some(last_seen) = &self.last_seen_at {
            writeln!(f, "Last Seen: {last_seen}")?;
        }
        if let Some(meta) = &self.metadata_json {
            writeln!(f, "Metadata:  {meta}")?;
        }
        writeln!(f, "Created:   {}", self.created_at)?;
        write!(f, "Updated:   {}", self.updated_at)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub agent_id: String,
    pub artifact_type: String,
    pub name: String,
    pub path: Option<String>,
    pub status: String,
    pub namespace: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_seen_at: Option<String>,
}

impl fmt::Display for Artifact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ID:        {}", self.id)?;
        writeln!(f, "Agent:     {}", self.agent_id)?;
        writeln!(f, "Type:      {}", self.artifact_type)?;
        writeln!(f, "Name:      {}", self.name)?;
        if let Some(path) = &self.path {
            writeln!(f, "Path:      {path}")?;
        }
        writeln!(f, "Status:    {}", self.status)?;
        writeln!(f, "Namespace: {}", self.namespace)?;
        writeln!(f, "Created:   {}", self.created_at)?;
        write!(f, "Updated:   {}", self.updated_at)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResult {
    pub agent: Agent,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum DeregisterResult {
    Deleted,
    Cascaded(i64),
    HasChildren(Vec<ChildInfo>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListResult {
    pub agents: Vec<Agent>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactListResult {
    pub artifacts: Vec<Artifact>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    #[serde(flatten)]
    pub agent: Agent,
    pub children: Vec<TreeNode>,
}
