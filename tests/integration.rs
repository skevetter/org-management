use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(db_path: &str) -> Command {
    let mut c = Command::cargo_bin("org-management").unwrap();
    c.arg("--db").arg(db_path);
    c
}

fn cmd_ns(db_path: &str, ns: &str) -> Command {
    let mut c = Command::cargo_bin("org-management").unwrap();
    c.arg("--db").arg(db_path).arg("--namespace").arg(ns);
    c
}

fn setup() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db").to_string_lossy().to_string();
    (dir, db_path)
}

#[test]
fn test_register_agent_text() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["register", "--name", "alice", "--type", "engineer"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Name:      alice"))
        .stdout(predicate::str::contains("Type:      engineer"));
}

#[test]
fn test_register_agent_json() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["--json", "register", "--name", "bob", "--type", "manager"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"bob\""))
        .stdout(predicate::str::contains("\"agent_type\":\"manager\""));
}

#[test]
fn test_register_upsert() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["register", "--name", "carol", "--type", "engineer"])
        .assert()
        .success();

    cmd(&db)
        .args(["register", "--name", "carol", "--type", "manager"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Type:      manager"));
}

#[test]
fn test_register_with_parent() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "parent-agent",
            "--type",
            "director",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let parent_id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "child-agent",
            "--type",
            "engineer",
            "--parent",
            parent_id,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"child-agent\""));
}

#[test]
fn test_register_with_metadata() {
    let (_dir, db) = setup();
    cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "meta-agent",
            "--type",
            "engineer",
            "--metadata",
            r#"{"team":"alpha"}"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"metadata_json\""));
}

#[test]
fn test_deregister_agent() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "to-remove",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args(["--json", "deregister", "--id", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("deregistered"));

    cmd(&db).args(["lookup", "--id", id]).assert().failure();
}

#[test]
fn test_lookup_by_id() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json", "register", "--name", "findme", "--type", "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args(["--json", "lookup", "--id", id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"findme\""));
}

#[test]
fn test_lookup_by_name() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["register", "--name", "namelookup", "--type", "manager"])
        .assert()
        .success();

    cmd(&db)
        .args(["--json", "lookup", "--name", "namelookup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"namelookup\""));
}

#[test]
fn test_lookup_not_found() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["lookup", "--name", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_lookup_requires_id_or_name() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["lookup"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--id or --name"));
}

#[test]
fn test_children_command() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args(["--json", "register", "--name", "boss", "--type", "director"])
        .output()
        .unwrap();
    let parent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "register", "--name", "worker1", "--type", "engineer", "--parent", parent_id,
        ])
        .assert()
        .success();
    cmd(&db)
        .args([
            "register", "--name", "worker2", "--type", "engineer", "--parent", parent_id,
        ])
        .assert()
        .success();

    cmd(&db)
        .args(["--json", "children", "--id", parent_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("worker1"))
        .stdout(predicate::str::contains("worker2"));
}

#[test]
fn test_children_json_total() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args(["--json", "register", "--name", "mgr", "--type", "manager"])
        .output()
        .unwrap();
    let parent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    for i in 0..3 {
        cmd(&db)
            .args([
                "register",
                "--name",
                &format!("eng{i}"),
                "--type",
                "engineer",
                "--parent",
                parent_id,
            ])
            .assert()
            .success();
    }

    let output = cmd(&db)
        .args(["--json", "children", "--id", parent_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["total"].as_i64().unwrap(), 3);
    assert_eq!(result["agents"].as_array().unwrap().len(), 3);
}

#[test]
fn test_children_pagination() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args(["--json", "register", "--name", "pager", "--type", "manager"])
        .output()
        .unwrap();
    let parent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    for i in 0..5 {
        cmd(&db)
            .args([
                "register",
                "--name",
                &format!("page-eng{i}"),
                "--type",
                "engineer",
                "--parent",
                parent_id,
            ])
            .assert()
            .success();
    }

    let output = cmd(&db)
        .args([
            "--json", "children", "--id", parent_id, "--limit", "2", "--offset", "0",
        ])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["agents"].as_array().unwrap().len(), 2);
    assert_eq!(result["total"].as_i64().unwrap(), 5);
}

#[test]
fn test_ancestors_command() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json", "register", "--name", "root-dir", "--type", "director",
        ])
        .output()
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let root_id = root["id"].as_str().unwrap();

    let output = cmd(&db)
        .args([
            "--json", "register", "--name", "mid-mgr", "--type", "manager", "--parent", root_id,
        ])
        .output()
        .unwrap();
    let mid: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let mid_id = mid["id"].as_str().unwrap();

    let output = cmd(&db)
        .args([
            "--json", "register", "--name", "leaf-eng", "--type", "engineer", "--parent", mid_id,
        ])
        .output()
        .unwrap();
    let leaf: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let leaf_id = leaf["id"].as_str().unwrap();

    let output = cmd(&db)
        .args(["--json", "ancestors", "--id", leaf_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ancestors = result["ancestors"].as_array().unwrap();
    assert_eq!(ancestors.len(), 2);
    assert_eq!(ancestors[0]["name"].as_str().unwrap(), "mid-mgr");
    assert_eq!(ancestors[1]["name"].as_str().unwrap(), "root-dir");
}

#[test]
fn test_tree_command() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "tree-root",
            "--type",
            "director",
        ])
        .output()
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let root_id = root["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "register",
            "--name",
            "tree-child",
            "--type",
            "engineer",
            "--parent",
            root_id,
        ])
        .assert()
        .success();

    cmd(&db)
        .args(["--json", "tree"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tree-root"))
        .stdout(predicate::str::contains("tree-child"));
}

#[test]
fn test_tree_with_root() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "subtree-root",
            "--type",
            "manager",
        ])
        .output()
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let root_id = root["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "register",
            "--name",
            "subtree-leaf",
            "--type",
            "engineer",
            "--parent",
            root_id,
        ])
        .assert()
        .success();

    let output = cmd(&db)
        .args(["--json", "tree", "--root", root_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let tree = result["tree"].as_array().unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0]["name"].as_str().unwrap(), "subtree-root");
    assert!(!tree[0]["children"].as_array().unwrap().is_empty());
}

#[test]
fn test_tree_text_output() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json", "register", "--name", "tt-root", "--type", "director",
        ])
        .output()
        .unwrap();
    let root: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let root_id = root["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "register", "--name", "tt-child", "--type", "engineer", "--parent", root_id,
        ])
        .assert()
        .success();

    cmd(&db)
        .args(["tree"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tt-root (director)"))
        .stdout(predicate::str::contains("tt-child (engineer)"));
}

#[test]
fn test_artifact_register() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "art-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "--json",
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "worktree",
            "--name",
            "feature-x",
            "--path",
            "/tmp/feature-x",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"feature-x\""))
        .stdout(predicate::str::contains("\"artifact_type\":\"worktree\""));
}

#[test]
fn test_artifact_register_upsert() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "upsert-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "--json",
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "branch",
            "--name",
            "main",
        ])
        .assert()
        .success();

    cmd(&db)
        .args([
            "--json",
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "branch",
            "--name",
            "main",
            "--path",
            "/new/path",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("/new/path"));
}

#[test]
fn test_artifact_list() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "list-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "room",
            "--name",
            "chat-room",
        ])
        .assert()
        .success();
    cmd(&db)
        .args([
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "schedule",
            "--name",
            "daily-check",
        ])
        .assert()
        .success();

    let output = cmd(&db)
        .args(["--json", "artifact", "list", "--agent-id", agent_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["total"].as_i64().unwrap(), 2);
}

#[test]
fn test_artifact_update_status() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "upd-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    let output = cmd(&db)
        .args([
            "--json",
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "worktree",
            "--name",
            "wt1",
        ])
        .output()
        .unwrap();
    let artifact: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let art_id = artifact["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "--json", "artifact", "update", "--id", art_id, "--status", "archived",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\":\"archived\""));
}

#[test]
fn test_artifact_deregister() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "dereg-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    let output = cmd(&db)
        .args([
            "--json",
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "branch",
            "--name",
            "old-branch",
        ])
        .output()
        .unwrap();
    let artifact: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let art_id = artifact["id"].as_str().unwrap();

    cmd(&db)
        .args(["--json", "artifact", "deregister", "--id", art_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("deregistered"));
}

#[test]
fn test_namespace_isolation_agents() {
    let (_dir, db) = setup();
    cmd_ns(&db, "ns-a")
        .args(["register", "--name", "isolated", "--type", "engineer"])
        .assert()
        .success();

    cmd_ns(&db, "ns-b")
        .args(["lookup", "--name", "isolated"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));

    cmd_ns(&db, "ns-a")
        .args(["--json", "lookup", "--name", "isolated"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"namespace\":\"ns-a\""));
}

#[test]
fn test_namespace_isolation_same_name() {
    let (_dir, db) = setup();
    cmd_ns(&db, "org1")
        .args([
            "--json",
            "register",
            "--name",
            "shared-name",
            "--type",
            "engineer",
        ])
        .assert()
        .success();

    cmd_ns(&db, "org2")
        .args([
            "--json",
            "register",
            "--name",
            "shared-name",
            "--type",
            "manager",
        ])
        .assert()
        .success();

    let output = cmd_ns(&db, "org1")
        .args(["--json", "lookup", "--name", "shared-name"])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(agent["agent_type"].as_str().unwrap(), "engineer");

    let output = cmd_ns(&db, "org2")
        .args(["--json", "lookup", "--name", "shared-name"])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(agent["agent_type"].as_str().unwrap(), "manager");
}

#[test]
fn test_namespace_isolation_artifacts() {
    let (_dir, db) = setup();
    let output = cmd_ns(&db, "ns-x")
        .args([
            "--json", "register", "--name", "ns-owner", "--type", "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    cmd_ns(&db, "ns-x")
        .args([
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "room",
            "--name",
            "my-room",
        ])
        .assert()
        .success();

    let output = cmd_ns(&db, "ns-x")
        .args(["--json", "artifact", "list", "--agent-id", agent_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["total"].as_i64().unwrap(), 1);
}

#[test]
fn test_cascade_delete_artifacts() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "cascade-test",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "worktree",
            "--name",
            "will-be-deleted",
        ])
        .assert()
        .success();

    cmd(&db)
        .args(["deregister", "--id", agent_id])
        .assert()
        .success();

    // Cannot list artifacts for a deleted agent
    cmd(&db)
        .args(["artifact", "list", "--agent-id", agent_id])
        .assert()
        .failure();
}

#[test]
fn test_cascade_delete_relationships() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "cascade-parent",
            "--type",
            "director",
        ])
        .output()
        .unwrap();
    let parent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "cascade-child",
            "--type",
            "engineer",
            "--parent",
            parent_id,
        ])
        .output()
        .unwrap();
    let child: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let child_id = child["id"].as_str().unwrap();

    cmd(&db)
        .args(["deregister", "--id", parent_id])
        .assert()
        .success();

    // Child still exists but has no ancestors
    let output = cmd(&db)
        .args(["--json", "ancestors", "--id", child_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["ancestors"].as_array().unwrap().len(), 0);
}

#[test]
fn test_short_id_prefix() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "prefix-test",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let full_id = agent["id"].as_str().unwrap();
    let short_id = &full_id[..8];

    cmd(&db)
        .args(["--json", "lookup", "--id", short_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"prefix-test\""));
}

#[test]
fn test_serve_starts_and_exits_on_closed_stdin() {
    let (_dir, db) = setup();
    cmd(&db)
        .args(["serve"])
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("ConnectionClosed"));
}

#[test]
fn test_default_namespace() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "default-ns-test",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(agent["namespace"].as_str().unwrap(), "default");
}

#[test]
fn test_agent_status_active_on_register() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "status-check",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(agent["status"].as_str().unwrap(), "active");
}

#[test]
fn test_artifact_list_pagination() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "paginator",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    for i in 0..4 {
        cmd(&db)
            .args([
                "artifact",
                "register",
                "--agent-id",
                agent_id,
                "--type",
                "branch",
                "--name",
                &format!("br-{i}"),
            ])
            .assert()
            .success();
    }

    let output = cmd(&db)
        .args([
            "--json",
            "artifact",
            "list",
            "--agent-id",
            agent_id,
            "--limit",
            "2",
            "--offset",
            "1",
        ])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["artifacts"].as_array().unwrap().len(), 2);
    assert_eq!(result["total"].as_i64().unwrap(), 4);
}

#[test]
fn test_register_all_agent_types() {
    let (_dir, db) = setup();
    for t in ["engineer", "manager", "director", "senior-manager"] {
        cmd(&db)
            .args([
                "--json",
                "register",
                "--name",
                &format!("type-{t}"),
                "--type",
                t,
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains(&format!("\"agent_type\":\"{t}\"")));
    }
}

#[test]
fn test_register_all_artifact_types() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "art-types-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    for t in ["worktree", "room", "schedule", "branch"] {
        cmd(&db)
            .args([
                "--json",
                "artifact",
                "register",
                "--agent-id",
                agent_id,
                "--type",
                t,
                "--name",
                &format!("a-{t}"),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains(&format!(
                "\"artifact_type\":\"{t}\""
            )));
    }
}

#[test]
fn test_children_empty() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json", "register", "--name", "loner", "--type", "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    let output = cmd(&db)
        .args(["--json", "children", "--id", agent_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["total"].as_i64().unwrap(), 0);
    assert_eq!(result["agents"].as_array().unwrap().len(), 0);
}

#[test]
fn test_ancestors_root_agent() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "no-parent",
            "--type",
            "director",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    let output = cmd(&db)
        .args(["--json", "ancestors", "--id", agent_id])
        .output()
        .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["ancestors"].as_array().unwrap().len(), 0);
}

#[test]
fn test_artifact_text_output() {
    let (_dir, db) = setup();
    let output = cmd(&db)
        .args([
            "--json",
            "register",
            "--name",
            "text-art-owner",
            "--type",
            "engineer",
        ])
        .output()
        .unwrap();
    let agent: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let agent_id = agent["id"].as_str().unwrap();

    cmd(&db)
        .args([
            "artifact",
            "register",
            "--agent-id",
            agent_id,
            "--type",
            "room",
            "--name",
            "my-text-room",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Name:      my-text-room"))
        .stdout(predicate::str::contains("Type:      room"));
}

#[test]
fn test_tree_empty() {
    let (_dir, db) = setup();
    cmd_ns(&db, "empty-ns")
        .args(["--json", "tree"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"tree\":[]"));
}

#[test]
fn test_actor_env_var() {
    let (_dir, db) = setup();
    cmd(&db)
        .env("PASEO_AGENT_ID", "test-actor-123")
        .args([
            "--json",
            "register",
            "--name",
            "env-actor-test",
            "--type",
            "engineer",
        ])
        .assert()
        .success();
}
