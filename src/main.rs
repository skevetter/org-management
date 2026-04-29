mod db;
mod mcp;
mod models;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use db::Database;
use mcp::server::OrgMcpServer;
use models::{AgentStatus, AgentType, ArtifactStatus, ArtifactType, DeregisterResult, TreeNode};

fn default_db_path() -> PathBuf {
    let base = match std::env::var("XDG_DATA_HOME") {
        Ok(val) if !val.is_empty() => {
            let path = PathBuf::from(&val);
            if path.is_relative() {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(path)
            } else {
                path
            }
        }
        _ => match std::env::var("HOME") {
            Ok(home) if !home.is_empty() => PathBuf::from(home).join(".local").join("share"),
            _ => PathBuf::from("."),
        },
    };
    base.join("org-management").join("org.db")
}

fn get_actor(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(|s| s.to_string())
        .or_else(|| std::env::var("PASEO_AGENT_ID").ok())
}

#[derive(Parser)]
#[command(
    name = "org-management",
    about = "Agent hierarchy and resource tracking",
    version
)]
struct Cli {
    #[arg(long, global = true)]
    db: Option<String>,

    #[arg(long, global = true)]
    json: bool,

    #[arg(long, short = 'n', global = true)]
    namespace: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Register {
        #[arg(long)]
        name: String,
        #[arg(long = "type", value_enum)]
        agent_type: AgentType,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        metadata: Option<String>,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long, value_enum)]
        status: Option<AgentStatus>,
        #[arg(long)]
        room: Option<String>,
    },
    Deregister {
        #[arg(long)]
        id: String,
        #[arg(long)]
        cascade: bool,
        #[arg(long)]
        actor: Option<String>,
    },
    Lookup {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    Children {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 50)]
        limit: i64,
        #[arg(long, default_value_t = 0)]
        offset: i64,
        #[arg(long, value_enum)]
        status: Option<AgentStatus>,
    },
    UpdateStatus {
        #[arg(long)]
        id: String,
        #[arg(long, value_enum)]
        status: AgentStatus,
    },
    Ancestors {
        #[arg(long)]
        id: String,
    },
    Tree {
        #[arg(long)]
        root: Option<String>,
    },
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommands,
    },
    Serve {
        #[arg(long, default_value = "stdio")]
        transport: String,
    },
}

#[derive(Subcommand)]
enum ArtifactCommands {
    Register {
        #[arg(long)]
        agent_id: String,
        #[arg(long = "type", value_enum)]
        artifact_type: ArtifactType,
        #[arg(long)]
        name: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        actor: Option<String>,
    },
    List {
        #[arg(long)]
        agent_id: String,
        #[arg(long, default_value_t = 50)]
        limit: i64,
        #[arg(long, default_value_t = 0)]
        offset: i64,
    },
    Update {
        #[arg(long)]
        id: String,
        #[arg(long, value_enum)]
        status: ArtifactStatus,
        #[arg(long)]
        actor: Option<String>,
    },
    Deregister {
        #[arg(long)]
        id: String,
        #[arg(long)]
        actor: Option<String>,
    },
}

fn print_tree_text(nodes: &[TreeNode], indent: usize) {
    for node in nodes {
        let prefix = "  ".repeat(indent);
        println!(
            "{}{} ({}) [{}]",
            prefix, node.agent.name, node.agent.agent_type, node.agent.id
        );
        print_tree_text(&node.children, indent + 1);
    }
}

fn main() {
    let cli = Cli::parse();
    let db_path = match cli.db {
        Some(p) => PathBuf::from(p),
        None => default_db_path(),
    };

    if let Some(parent) = db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
            eprintln!("Failed to create database directory: {e}");
            std::process::exit(1);
        });
    }

    let db_str = db_path.to_string_lossy();
    let db = Database::open(&db_str).unwrap_or_else(|e| {
        eprintln!("Failed to open database: {e}");
        std::process::exit(1);
    });

    let json = cli.json;
    let namespace = cli.namespace.as_deref().unwrap_or("default");

    match cli.command {
        Commands::Register {
            name,
            agent_type,
            parent,
            metadata,
            actor,
            status,
            room,
        } => {
            let actor = get_actor(actor.as_deref());
            let status_str = status.map(|s| s.to_string());
            let result = db
                .register_agent(
                    &name,
                    &agent_type.to_string(),
                    parent.as_deref(),
                    namespace,
                    metadata.as_deref(),
                    actor.as_deref(),
                    status_str.as_deref(),
                    room.as_deref(),
                )
                .unwrap_or_else(|e| {
                    eprintln!("Failed to register agent: {e}");
                    std::process::exit(1);
                });
            let action = if result.created { "created" } else { "updated" };
            if json {
                let mut val = serde_json::to_value(&result.agent).unwrap();
                val.as_object_mut()
                    .unwrap()
                    .insert("action".to_string(), serde_json::json!(action));
                println!("{}", serde_json::to_string(&val).unwrap());
            } else {
                let label = if result.created { "CREATED" } else { "UPDATED" };
                println!("{label} {}", result.agent.name);
                println!("{}", result.agent);
            }
        }
        Commands::Deregister { id, cascade, actor } => {
            let actor = get_actor(actor.as_deref());
            let result = db
                .deregister_agent(&id, namespace, actor.as_deref(), cascade)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to deregister agent: {e}");
                    std::process::exit(1);
                });
            match result {
                DeregisterResult::HasChildren(children) => {
                    if json {
                        let child_list: Vec<_> = children
                            .iter()
                            .map(|c| serde_json::json!({"id": c.id, "name": c.name}))
                            .collect();
                        eprintln!(
                            "{}",
                            serde_json::to_string(&serde_json::json!({
                                "error": "has_children",
                                "children": child_list
                            }))
                            .unwrap()
                        );
                    } else {
                        eprintln!(
                            "Cannot deregister {}: has {} children. Use --cascade to delete subtree.",
                            id,
                            children.len()
                        );
                    }
                    std::process::exit(1);
                }
                DeregisterResult::Cascaded(count) => {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string(
                                &serde_json::json!({"deregistered": count, "cascade": true})
                            )
                            .unwrap()
                        );
                    } else {
                        println!("Deregistered {count} agents (cascade).");
                    }
                }
                DeregisterResult::Deleted => {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string(&serde_json::json!({"deregistered": id}))
                                .unwrap()
                        );
                    } else {
                        println!("Agent {id} deregistered.");
                    }
                }
            }
        }
        Commands::Lookup { id, name } => {
            if id.is_none() && name.is_none() {
                eprintln!("Must provide either --id or --name");
                std::process::exit(1);
            }
            let agent = db
                .lookup_agent(id.as_deref(), name.as_deref(), namespace)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to lookup agent: {e}");
                    std::process::exit(1);
                });
            match agent {
                Some(a) => {
                    if json {
                        println!("{}", serde_json::to_string(&a).unwrap());
                    } else {
                        println!("{a}");
                    }
                }
                None => {
                    eprintln!("Agent not found");
                    std::process::exit(1);
                }
            }
        }
        Commands::Children {
            id,
            limit,
            offset,
            status,
        } => {
            let status_str = status.map(|s| s.to_string());
            let result = db
                .list_children(&id, namespace, limit, offset, status_str.as_deref())
                .unwrap_or_else(|e| {
                    eprintln!("Failed to list children: {e}");
                    std::process::exit(1);
                });
            if json {
                println!("{}", serde_json::to_string(&result).unwrap());
            } else if result.agents.is_empty() {
                println!("No children found.");
            } else {
                println!("{:<38} {:<20} {:<12}", "ID", "NAME", "TYPE");
                println!("{}", "-".repeat(70));
                for a in &result.agents {
                    println!("{:<38} {:<20} {:<12}", a.id, a.name, a.agent_type);
                }
                println!("\n{} agent(s) total", result.total);
            }
        }
        Commands::UpdateStatus { id, status } => {
            let agent = db
                .update_agent_status(&id, &status.to_string(), namespace)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to update agent status: {e}");
                    std::process::exit(1);
                });
            if json {
                println!("{}", serde_json::to_string(&agent).unwrap());
            } else {
                println!("{agent}");
            }
        }
        Commands::Ancestors { id } => {
            let ancestors = db.list_ancestors(&id, namespace).unwrap_or_else(|e| {
                eprintln!("Failed to list ancestors: {e}");
                std::process::exit(1);
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({"ancestors": ancestors})).unwrap()
                );
            } else if ancestors.is_empty() {
                println!("No ancestors found (agent is a root).");
            } else {
                for (i, a) in ancestors.iter().enumerate() {
                    let prefix = "  ".repeat(i);
                    println!("{}{} ({}) [{}]", prefix, a.name, a.agent_type, a.id);
                }
            }
        }
        Commands::Tree { root } => {
            let tree = db.get_tree(root.as_deref(), namespace).unwrap_or_else(|e| {
                eprintln!("Failed to build tree: {e}");
                std::process::exit(1);
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({"tree": tree})).unwrap()
                );
            } else if tree.is_empty() {
                println!("No agents found.");
            } else {
                print_tree_text(&tree, 0);
            }
        }
        Commands::Artifact { command } => match command {
            ArtifactCommands::Register {
                agent_id,
                artifact_type,
                name,
                path,
                actor,
            } => {
                let actor = get_actor(actor.as_deref());
                let artifact = db
                    .register_artifact(
                        &agent_id,
                        &artifact_type.to_string(),
                        &name,
                        path.as_deref(),
                        namespace,
                        actor.as_deref(),
                    )
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to register artifact: {e}");
                        std::process::exit(1);
                    });
                if json {
                    println!("{}", serde_json::to_string(&artifact).unwrap());
                } else {
                    println!("{artifact}");
                }
            }
            ArtifactCommands::List {
                agent_id,
                limit,
                offset,
            } => {
                let result = db
                    .list_artifacts(&agent_id, namespace, limit, offset)
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to list artifacts: {e}");
                        std::process::exit(1);
                    });
                if json {
                    println!("{}", serde_json::to_string(&result).unwrap());
                } else if result.artifacts.is_empty() {
                    println!("No artifacts found.");
                } else {
                    println!(
                        "{:<38} {:<12} {:<20} {:<10}",
                        "ID", "TYPE", "NAME", "STATUS"
                    );
                    println!("{}", "-".repeat(80));
                    for art in &result.artifacts {
                        println!(
                            "{:<38} {:<12} {:<20} {:<10}",
                            art.id, art.artifact_type, art.name, art.status
                        );
                    }
                    println!("\n{} artifact(s) total", result.total);
                }
            }
            ArtifactCommands::Update { id, status, actor } => {
                let actor = get_actor(actor.as_deref());
                let artifact = db
                    .update_artifact(&id, &status.to_string(), namespace, actor.as_deref())
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to update artifact: {e}");
                        std::process::exit(1);
                    });
                match artifact {
                    Some(a) => {
                        if json {
                            println!("{}", serde_json::to_string(&a).unwrap());
                        } else {
                            println!("{a}");
                        }
                    }
                    None => {
                        eprintln!("Artifact not found: {id}");
                        std::process::exit(1);
                    }
                }
            }
            ArtifactCommands::Deregister { id, actor } => {
                let actor = get_actor(actor.as_deref());
                db.deregister_artifact(&id, namespace, actor.as_deref())
                    .unwrap_or_else(|e| {
                        eprintln!("Failed to deregister artifact: {e}");
                        std::process::exit(1);
                    });
                if json {
                    println!(
                        "{}",
                        serde_json::to_string(&serde_json::json!({"deregistered": id})).unwrap()
                    );
                } else {
                    println!("Artifact {id} deregistered.");
                }
            }
        },
        Commands::Serve { transport } => {
            if transport != "stdio" {
                eprintln!("Only stdio transport is supported");
                std::process::exit(1);
            }
            let actor = std::env::var("PASEO_AGENT_ID").ok();
            let ns = cli.namespace;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                use rmcp::ServiceExt;
                let server = OrgMcpServer::new(db, ns, actor);
                let transport = rmcp::transport::io::stdio();
                let service = server.serve(transport).await.unwrap();
                service.waiting().await.unwrap();
            });
        }
    }
}
