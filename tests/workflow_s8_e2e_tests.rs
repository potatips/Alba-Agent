use std::path::PathBuf;
use tokio::process::Command;

use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::store::CheckpointStore;
use nexus_albaai::domain::ExecutionScope;
use tempfile::tempdir;
use uuid::Uuid;

mod common;
use common::{MockResponse, TestHttpServer};

#[tokio::test]
async fn test_three_agent_workflow_e2e_cli_and_sqlite_verifications() {
    let bin_path = env!("CARGO_BIN_EXE_nexus-albaai");
    let server = TestHttpServer::start().await;
    server.enqueue_response(MockResponse::final_answer(
        "Telemetry report compiled: system stable",
    ));

    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("e2e_workflow.db");
    let workflow_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/workflows/three_agent_approval.json");
    let run_id = Uuid::new_v4().to_string();

    let run_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_test_key")
        .env("NEXUS_ACTOR_ID", "author_alice")
        .args([
            "run",
            "--workflow",
            workflow_path.to_str().unwrap(),
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--model",
            "gpt-4o",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        run_out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&run_out.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run_out.stdout);
    assert!(run_stdout.contains("Workflow run paused for human approval."));
    assert!(run_stdout.contains("Node: approval_gate"));

    let approval_line = run_stdout
        .lines()
        .find(|l| l.starts_with("Approval ID: "))
        .expect("missing Approval ID in run output");
    let approval_id = approval_line
        .trim_start_matches("Approval ID: ")
        .trim()
        .to_string();

    let status_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(status_out.status.success());
    let status_str = String::from_utf8_lossy(&status_out.stdout);
    assert!(status_str.contains("Status: awaiting_approval"));
    assert!(status_str.contains("Node: approval_gate"));
    assert!(status_str.contains(&format!("Pending approval ID: {}", approval_id)));

    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");
    }

    let self_approve_out = Command::new(bin_path)
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            &approval_id,
            "--actor",
            "author_alice",
            "--decision",
            "allow_once",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(!self_approve_out.status.success());
    let self_err = String::from_utf8_lossy(&self_approve_out.stderr);
    assert!(self_err.contains("Separation of duties violation"));

    let valid_approve_out = Command::new(bin_path)
        .args([
            "approve",
            "--run-id",
            &run_id,
            "--approval-id",
            &approval_id,
            "--actor",
            "approver_bob",
            "--decision",
            "allow_once",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(
        valid_approve_out.status.success(),
        "approve failed: {}",
        String::from_utf8_lossy(&valid_approve_out.stderr)
    );

    server.enqueue_response(MockResponse::final_answer(
        "Production release deployed successfully.",
    ));
    server.enqueue_response(MockResponse::final_answer(
        "Verification health checklist passed.",
    ));

    let requests_before_resume = server.received_requests().len();

    let resume_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_test_key")
        .env("NEXUS_ACTOR_ID", "author_alice")
        .args([
            "resume",
            "--workflow",
            workflow_path.to_str().unwrap(),
            "--run-id",
            &run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--model",
            "gpt-4o",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        resume_out.status.success(),
        "resume failed: {}",
        String::from_utf8_lossy(&resume_out.stderr)
    );
    let resume_stdout = String::from_utf8_lossy(&resume_out.stdout);
    assert!(resume_stdout.contains("Workflow run resumed and completed successfully"));

    let requests_after_resume = server.received_requests().len();
    assert_eq!(requests_after_resume - requests_before_resume, 2);

    let final_status_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(final_status_out.status.success());
    let final_status_str = String::from_utf8_lossy(&final_status_out.stdout);
    assert!(final_status_str.contains("Status: completed"));

    let fork_out = Command::new(bin_path)
        .args([
            "fork",
            "--run-id",
            &run_id,
            "--sequence",
            "4",
            "--branch-label",
            "experiment-branch",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(
        fork_out.status.success(),
        "fork failed: {}",
        String::from_utf8_lossy(&fork_out.stderr)
    );
    let fork_stdout = String::from_utf8_lossy(&fork_out.stdout);
    assert!(fork_stdout.contains("Forked workflow run "));
    let forked_id_part = fork_stdout
        .split("Forked workflow run ")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap();
    let forked_run_id = forked_id_part.trim().to_string();

    let resume_fork_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_test_key")
        .env("NEXUS_ACTOR_ID", "author_alice")
        .args([
            "resume",
            "--workflow",
            workflow_path.to_str().unwrap(),
            "--run-id",
            &forked_run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--model",
            "gpt-4o",
        ])
        .output()
        .await
        .unwrap();
    assert!(resume_fork_out.status.success());
    let resume_fork_stdout = String::from_utf8_lossy(&resume_fork_out.stdout);
    assert!(resume_fork_stdout.contains("Workflow run is still paused waiting for approval."));

    let forked_status_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &forked_run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(forked_status_out.status.success());
    let forked_status_str = String::from_utf8_lossy(&forked_status_out.stdout);
    assert!(forked_status_str.contains("Status: awaiting_approval"));
    let forked_approval_line = forked_status_str
        .lines()
        .find(|l| l.starts_with("Pending approval ID: "))
        .expect("missing Pending approval ID for forked run");
    let forked_approval_id = forked_approval_line
        .trim_start_matches("Pending approval ID: ")
        .trim()
        .to_string();

    let deny_out = Command::new(bin_path)
        .args([
            "deny",
            "--run-id",
            &forked_run_id,
            "--approval-id",
            &forked_approval_id,
            "--actor",
            "approver_bob",
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(
        deny_out.status.success(),
        "deny failed: {}",
        String::from_utf8_lossy(&deny_out.stderr)
    );

    let resume_denied_fork_out = Command::new(bin_path)
        .env("NEXUS_API_KEY", "cli_test_key")
        .env("NEXUS_ACTOR_ID", "author_alice")
        .args([
            "resume",
            "--workflow",
            workflow_path.to_str().unwrap(),
            "--run-id",
            &forked_run_id,
            "--workspace-root",
            ws_dir.path().to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--api-base",
            &server.url(),
            "--model",
            "gpt-4o",
        ])
        .output()
        .await
        .unwrap();
    let resume_denied_stderr = String::from_utf8_lossy(&resume_denied_fork_out.stderr);
    assert!(resume_denied_stderr.contains("Workflow approval denied by operator"));

    let forked_final_status_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &forked_run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(forked_final_status_out.status.success());
    let forked_final_str = String::from_utf8_lossy(&forked_final_status_out.stdout);
    assert!(forked_final_str.contains("Status: failed"));

    let parent_check_out = Command::new(bin_path)
        .args([
            "status",
            "--run-id",
            &run_id,
            "--db-path",
            db_path.to_str().unwrap(),
        ])
        .output()
        .await
        .unwrap();
    assert!(parent_check_out.status.success());
    let parent_check_str = String::from_utf8_lossy(&parent_check_out.stdout);
    assert!(parent_check_str.contains("Status: completed"));

    let conn = rusqlite::Connection::open(&db_path).unwrap();

    let parent_checkpoint_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM workflow_checkpoints WHERE run_id = ?1",
            rusqlite::params![run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(parent_checkpoint_count, 10);

    let forked_checkpoint_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM workflow_checkpoints WHERE run_id = ?1",
            rusqlite::params![forked_run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(forked_checkpoint_count, 6);

    let (parent_app_status, parent_app_author, parent_app_decided): (
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT status, author_id, decided_by FROM approvals WHERE id = ?1",
            rusqlite::params![approval_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(parent_app_status, "consumed");
    assert_eq!(parent_app_author.as_deref(), Some("author_alice"));
    assert_eq!(parent_app_decided.as_deref(), Some("approver_bob"));

    let (forked_app_status, forked_app_author, forked_app_decided): (
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT status, author_id, decided_by FROM approvals WHERE id = ?1",
            rusqlite::params![forked_approval_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(forked_app_status, "denied");
    assert_eq!(forked_app_author.as_deref(), Some("author_alice"));
    assert_eq!(forked_app_decided.as_deref(), Some("approver_bob"));

    let self_rej_audit: i64 = conn
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = 'approval.self_approval_rejected' AND actor_id = 'author_alice' AND target_id = ?1",
            rusqlite::params![approval_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(self_rej_audit, 1);

    let parent_resolved_audit: i64 = conn
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = 'approval.resolved' AND actor_id = 'approver_bob' AND target_id = ?1",
            rusqlite::params![approval_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(parent_resolved_audit, 1);

    let forked_denied_audit: i64 = conn
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = 'approval.resolved' AND actor_id = 'approver_bob' AND target_id = ?1",
            rusqlite::params![forked_approval_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(forked_denied_audit, 1);

    let (branch_parent, branch_forked, branch_seq, branch_label): (String, String, u32, String) =
        conn.query_row(
            "SELECT parent_run_id, forked_run_id, fork_sequence, branch_label FROM trajectory_branches WHERE forked_run_id = ?1",
            rusqlite::params![forked_run_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(branch_parent, run_id);
    assert_eq!(branch_forked, forked_run_id);
    assert_eq!(branch_seq, 4);
    assert_eq!(branch_label, "experiment-branch");
}

#[tokio::test]
async fn test_sigkill_flakiness_20_iterations() {
    let mut pass_count = 0;
    for iteration in 0..20 {
        let db_dir = tempdir().unwrap();
        let db_path = db_dir.path().join(format!("flakiness_{}.db", iteration));

        let scope = ExecutionScope::new(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()).unwrap();

        {
            let store = SqliteCheckpointStore::new(&db_path).unwrap();
            store
                .record_tool_call(
                    &scope,
                    "node_research",
                    &format!("call_{}", iteration),
                    "system_telemetry",
                    &serde_json::json!({ "iteration": iteration }),
                )
                .await
                .unwrap();
            store
                .complete_tool_call(
                    &scope,
                    "node_research",
                    &format!("call_{}", iteration),
                    &format!("result_{}", iteration),
                )
                .await
                .unwrap();
        }

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let integrity: String = conn
            .query_row("PRAGMA integrity_check;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");

        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");

        let reopened_store = SqliteCheckpointStore::new(&db_path).unwrap();
        let journal_entry = reopened_store
            .get_tool_call_status(&scope, "node_research", &format!("call_{}", iteration))
            .await
            .unwrap()
            .expect("journal entry must exist after abrupt termination");
        assert_eq!(journal_entry.0, "consumed");
        assert_eq!(
            journal_entry.1.as_deref(),
            Some(format!("result_{}", iteration).as_str())
        );

        pass_count += 1;
    }
    assert_eq!(pass_count, 20);
}
