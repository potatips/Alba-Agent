use std::collections::BTreeMap;
use std::sync::Arc;

use axum::http::StatusCode;
use chrono::Utc;
use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::types::{ArtifactRecord, RunEvidenceRecord, RunRecord};
use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::events::sink::InMemoryEventSink;
use nexus_albaai::governance::policy::DefaultApprovalPolicy;
use nexus_albaai::models::adapter::ModelClient;
use nexus_albaai::models::types::{ModelError, ModelRequest, ModelResponse};
use nexus_albaai::sandbox::{SandboxPolicy, SandboxRunner};
use nexus_albaai::schema::{
    AgentSpec, ConsensusConfigSpec, CrewNodeSpec, CrewProcess, EndNodeSpec, JoinMode,
    StartNodeSpec, TaskSpec, WorkflowEdgeSpec, WorkflowModesSpec, WorkflowNode, WorkflowSpec,
};
use nexus_albaai::server::create_app;
use nexus_albaai::server::state::AppState;
use nexus_albaai::tools::types::ToolRegistry;
use nexus_albaai::workflow::engine::{WorkflowExecutor, WorkflowOutcome};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use uuid::Uuid;

struct DummyModelClient;

impl ModelClient for DummyModelClient {
    async fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelError> {
        Ok(ModelResponse {
            content: Some("Task finished successfully.".to_string()),
            tool_calls: vec![],
            usage: None,
        })
    }
}

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn test_verification_command_nonzero_exit_cannot_be_marked_verified() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("evidence_test_1.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let run_id = Uuid::new_v4().to_string();
    let wf_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id,
        status: "COMPLETED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now.clone()),
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let evidence_record = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "cargo test --test integration".to_string(),
        exit_code: 101,
        stdout: "FAILED: 1 test failed".to_string(),
        stderr: "panic occurred".to_string(),
        duration_ms: 1250,
        verified: false,
        created_at: now,
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&evidence_record).unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(
        !marked,
        "A run whose verification command exited non-zero must not be marked verified"
    );

    let fetched = store.get_run(&run_id).unwrap().expect("run must exist");
    assert!(!fetched.verified);
    assert!(!store.is_run_verified(&run_id).unwrap());
}

#[tokio::test]
async fn test_verification_command_zero_exit_can_be_marked_verified() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("evidence_test_2.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let run_id = Uuid::new_v4().to_string();
    let wf_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id,
        status: "COMPLETED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now.clone()),
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let art_path = db_dir.path().join("report.txt");
    std::fs::write(&art_path, b"test report content").unwrap();
    let art_hash = compute_sha256(b"test report content");

    let artifact = ArtifactRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        filename: "report.txt".to_string(),
        file_path: art_path.to_string_lossy().to_string(),
        file_size: 19,
        mime_type: "text/plain".to_string(),
        sha256: art_hash,
        verified: true,
        created_at: now.clone(),
    };
    store.insert_artifact(&artifact).unwrap();

    let evidence_record = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "cargo test --test integration".to_string(),
        exit_code: 0,
        stdout: "test result: ok. 12 passed; 0 failed".to_string(),
        stderr: "".to_string(),
        duration_ms: 840,
        verified: true,
        created_at: now,
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&evidence_record).unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(
        marked,
        "A run whose verification commands all succeeded must be marked verified"
    );

    let fetched = store.get_run(&run_id).unwrap().expect("run must exist");
    assert!(fetched.verified);
    assert!(store.is_run_verified(&run_id).unwrap());
}

#[tokio::test]
async fn test_subsequent_nonzero_command_invalidates_verified_status() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("evidence_test_3.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let run_id = Uuid::new_v4().to_string();
    let wf_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id,
        status: "COMPLETED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now.clone()),
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let art_path = db_dir.path().join("suite.txt");
    std::fs::write(&art_path, b"test suite output").unwrap();
    let art_hash = compute_sha256(b"test suite output");

    let artifact = ArtifactRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        filename: "suite.txt".to_string(),
        file_path: art_path.to_string_lossy().to_string(),
        file_size: 17,
        mime_type: "text/plain".to_string(),
        sha256: art_hash,
        verified: true,
        created_at: now.clone(),
    };
    store.insert_artifact(&artifact).unwrap();

    let success_evidence = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "cargo test --test part1".to_string(),
        exit_code: 0,
        stdout: "ok".to_string(),
        stderr: "".to_string(),
        duration_ms: 400,
        verified: true,
        created_at: now.clone(),
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&success_evidence).unwrap();
    assert!(store.mark_run_verified(&run_id, true).unwrap());
    assert!(store.is_run_verified(&run_id).unwrap());

    let failure_evidence = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "cargo test --test part2".to_string(),
        exit_code: 2,
        stdout: "error in suite".to_string(),
        stderr: "exit code 2".to_string(),
        duration_ms: 550,
        verified: false,
        created_at: Utc::now().to_rfc3339(),
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&failure_evidence).unwrap();

    assert!(
        !store.is_run_verified(&run_id).unwrap(),
        "Subsequent non-zero verification must immediately clear verified flag"
    );

    let re_attempt = store.mark_run_verified(&run_id, true).unwrap();
    assert!(
        !re_attempt,
        "Cannot re-verify when any evidence has non-zero exit code"
    );
}

#[tokio::test]
async fn test_unverified_if_no_evidence_records_exist() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("evidence_test_4.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let run_id = Uuid::new_v4().to_string();
    let wf_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id,
        status: "COMPLETED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now),
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(
        !marked,
        "Cannot mark verified if no verification commands have run"
    );
    assert!(!store.is_run_verified(&run_id).unwrap());
}

#[tokio::test]
async fn test_evidence_persistence_and_retrieval() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("evidence_test_5.db");
    let store = SqliteCheckpointStore::new(&db_path).unwrap();

    let run_id = Uuid::new_v4().to_string();
    let wf_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id,
        status: "COMPLETED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now.clone()),
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let ev1 = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "cargo check".to_string(),
        exit_code: 0,
        stdout: "Finished check".to_string(),
        stderr: "".to_string(),
        duration_ms: 210,
        verified: true,
        created_at: "2026-11-12T15:00:00Z".to_string(),
        output_sha256: String::new(),
        unverified_reason: None,
    };
    let ev2 = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "cargo test".to_string(),
        exit_code: 0,
        stdout: "test result: ok".to_string(),
        stderr: "".to_string(),
        duration_ms: 780,
        verified: true,
        created_at: "2026-11-12T15:01:00Z".to_string(),
        output_sha256: String::new(),
        unverified_reason: None,
    };

    store.insert_run_evidence(&ev1).unwrap();
    store.insert_run_evidence(&ev2).unwrap();

    let list = store.list_run_evidence(&run_id).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].command, "cargo check");
    assert_eq!(list[1].command, "cargo test");
    assert_eq!(list[0].duration_ms, 210);
    assert_eq!(list[1].duration_ms, 780);
    assert!(!list[0].output_sha256.is_empty());
    assert!(!list[1].output_sha256.is_empty());
}

#[tokio::test]
async fn test_evidence_surfaced_via_api_endpoints() {
    let ws_dir = tempdir().unwrap();
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().join("evidence_api.db");
    let store = Arc::new(SqliteCheckpointStore::new(&db_path).unwrap());

    let run_id = Uuid::new_v4().to_string();
    let wf_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let run_rec = RunRecord {
        id: run_id.clone(),
        workflow_id: wf_id,
        status: "COMPLETED".to_string(),
        input_data: "{}".to_string(),
        output_data: Some("{}".to_string()),
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: Some(now.clone()),
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let art_path = ws_dir.path().join("pytest_results.xml");
    let content = b"<testsuite tests=\"5\"></testsuite>";
    std::fs::write(&art_path, content).unwrap();
    let art_hash = compute_sha256(content);

    let artifact = ArtifactRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        filename: "pytest_results.xml".to_string(),
        file_path: art_path.to_string_lossy().to_string(),
        file_size: content.len() as i64,
        mime_type: "application/xml".to_string(),
        sha256: art_hash,
        verified: true,
        created_at: now.clone(),
    };
    store.insert_artifact(&artifact).unwrap();

    let ev = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "pytest tests/".to_string(),
        exit_code: 0,
        stdout: "5 passed".to_string(),
        stderr: "".to_string(),
        duration_ms: 350,
        verified: true,
        created_at: now,
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&ev).unwrap();
    store.mark_run_verified(&run_id, true).unwrap();

    let app_state = AppState::new(store.clone(), ws_dir.path().to_path_buf());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, create_app(app_state)).await.unwrap();
    });

    let client = reqwest::Client::new();

    let auth_resp = client
        .post(format!("http://{}/api/v1/auth/local-session", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(auth_resp.status(), StatusCode::OK);
    let auth_json: Value = auth_resp.json().await.unwrap();
    let token = auth_json["access_token"].as_str().unwrap().to_string();

    let run_resp = client
        .get(format!("http://{}/api/v1/runs/{}", addr, run_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(run_resp.status(), StatusCode::OK);
    let run_json: Value = run_resp.json().await.unwrap();
    assert_eq!(run_json["verified"], true);

    let evidence_resp = client
        .get(format!("http://{}/api/v1/runs/{}/evidence", addr, run_id))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();

    assert_eq!(evidence_resp.status(), StatusCode::OK);
    let evidence_json: Value = evidence_resp.json().await.unwrap();
    assert!(evidence_json.is_array());
    let arr = evidence_json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["command"], "pytest tests/");
    assert_eq!(arr[0]["exit_code"], 0);
    assert_eq!(arr[0]["verified"], true);
    assert!(!arr[0]["output_sha256"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_real_command_false_and_exit_3_stored_as_unverified() {
    let ws = tempdir().unwrap();
    let db = tempdir().unwrap();
    let store = SqliteCheckpointStore::new(db.path().join("real_cmd.db")).unwrap();

    let policy = SandboxPolicy::new(ws.path());
    let false_out = SandboxRunner::execute(
        &policy,
        "/bin/sh",
        &["-c".to_string(), "false".to_string()],
        None,
    )
    .await
    .unwrap();
    assert_ne!(false_out.exit_code, 0);

    let exit3_out = SandboxRunner::execute(
        &policy,
        "/bin/sh",
        &["-c".to_string(), "exit 3".to_string()],
        None,
    )
    .await
    .unwrap();
    assert_eq!(exit3_out.exit_code, 3);

    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    store
        .insert_run(&RunRecord {
            id: run_id.clone(),
            workflow_id: Uuid::new_v4().to_string(),
            status: "COMPLETED".to_string(),
            input_data: "{}".to_string(),
            output_data: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: Some(now.clone()),
            verified: false,
            unverified_reason: None,
        })
        .unwrap();

    let ev = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "exit 3".to_string(),
        exit_code: exit3_out.exit_code,
        stdout: exit3_out.stdout,
        stderr: exit3_out.stderr,
        duration_ms: exit3_out.duration_ms as i64,
        verified: false,
        created_at: now,
        output_sha256: String::new(),
        unverified_reason: Some("Verification command exited with status 3".to_string()),
    };
    store.insert_run_evidence(&ev).unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(!marked);

    let run = store.get_run(&run_id).unwrap().unwrap();
    assert!(!run.verified);
    assert!(run.unverified_reason.as_deref().unwrap_or("").contains("3"));

    let records = store.list_run_evidence(&run_id).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].exit_code, 3);
    assert!(!records[0].verified);
    assert!(!records[0].output_sha256.is_empty());
}

#[tokio::test]
async fn test_real_command_true_with_invalid_artifact_unverified() {
    let ws = tempdir().unwrap();
    let db = tempdir().unwrap();
    let store = SqliteCheckpointStore::new(db.path().join("invalid_art.db")).unwrap();

    let policy = SandboxPolicy::new(ws.path());
    let true_out = SandboxRunner::execute(
        &policy,
        "/bin/sh",
        &["-c".to_string(), "true".to_string()],
        None,
    )
    .await
    .unwrap();
    assert_eq!(true_out.exit_code, 0);

    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    store
        .insert_run(&RunRecord {
            id: run_id.clone(),
            workflow_id: Uuid::new_v4().to_string(),
            status: "COMPLETED".to_string(),
            input_data: "{}".to_string(),
            output_data: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: Some(now.clone()),
            verified: false,
            unverified_reason: None,
        })
        .unwrap();

    let ev = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "true".to_string(),
        exit_code: true_out.exit_code,
        stdout: true_out.stdout,
        stderr: true_out.stderr,
        duration_ms: true_out.duration_ms as i64,
        verified: true,
        created_at: now.clone(),
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&ev).unwrap();

    let missing_art = ArtifactRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        filename: "non_existent.pdf".to_string(),
        file_path: ws
            .path()
            .join("non_existent.pdf")
            .to_string_lossy()
            .to_string(),
        file_size: 100,
        mime_type: "application/pdf".to_string(),
        sha256: "deadbeef".to_string(),
        verified: false,
        created_at: now,
    };
    store.insert_artifact(&missing_art).unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(!marked);

    let run = store.get_run(&run_id).unwrap().unwrap();
    assert!(!run.verified);
    assert!(run.unverified_reason.is_some());
}

#[tokio::test]
async fn test_real_command_true_with_valid_artifact_verified_hashes_stored() {
    let ws = tempdir().unwrap();
    let db = tempdir().unwrap();
    let store = SqliteCheckpointStore::new(db.path().join("valid_hashes.db")).unwrap();

    let art_file = ws.path().join("verified_deliverable.txt");
    let content = b"authentic verified deliverable content from agent team";
    std::fs::write(&art_file, content).unwrap();
    let art_sha = compute_sha256(content);

    let policy = SandboxPolicy::new(ws.path());
    let real_out = SandboxRunner::execute(
        &policy,
        "/bin/sh",
        &["-c".to_string(), "echo verified_ok".to_string()],
        None,
    )
    .await
    .unwrap();
    assert_eq!(real_out.exit_code, 0);

    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    store
        .insert_run(&RunRecord {
            id: run_id.clone(),
            workflow_id: Uuid::new_v4().to_string(),
            status: "COMPLETED".to_string(),
            input_data: "{}".to_string(),
            output_data: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: Some(now.clone()),
            verified: false,
            unverified_reason: None,
        })
        .unwrap();

    let artifact = ArtifactRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        filename: "verified_deliverable.txt".to_string(),
        file_path: art_file.to_string_lossy().to_string(),
        file_size: content.len() as i64,
        mime_type: "text/plain".to_string(),
        sha256: art_sha.clone(),
        verified: true,
        created_at: now.clone(),
    };
    store.insert_artifact(&artifact).unwrap();

    let ev = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "echo verified_ok".to_string(),
        exit_code: real_out.exit_code,
        stdout: real_out.stdout.clone(),
        stderr: real_out.stderr.clone(),
        duration_ms: real_out.duration_ms as i64,
        verified: true,
        created_at: now,
        output_sha256: RunEvidenceRecord::compute_output_sha256(&real_out.stdout, &real_out.stderr),
        unverified_reason: None,
    };
    store.insert_run_evidence(&ev).unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(marked);

    let run = store.get_run(&run_id).unwrap().unwrap();
    assert!(run.verified);
    assert!(run.unverified_reason.is_none());

    let stored_arts = store.list_artifacts(&run_id).unwrap();
    assert_eq!(stored_arts.len(), 1);
    assert_eq!(stored_arts[0].sha256, art_sha);
    assert_eq!(stored_arts[0].file_size, content.len() as i64);

    let stored_evs = store.list_run_evidence(&run_id).unwrap();
    assert_eq!(stored_evs.len(), 1);
    assert_eq!(stored_evs[0].exit_code, 0);
    assert_eq!(
        stored_evs[0].output_sha256,
        RunEvidenceRecord::compute_output_sha256(&real_out.stdout, &real_out.stderr)
    );
}

#[tokio::test]
async fn test_artifact_replaced_after_verification_fails_recheck_persists_reason() {
    let ws = tempdir().unwrap();
    let db = tempdir().unwrap();
    let store = SqliteCheckpointStore::new(db.path().join("replaced.db")).unwrap();

    let art_file = ws.path().join("contract.pdf");
    let original = b"%PDF-1.4 original contract content";
    std::fs::write(&art_file, original).unwrap();
    let original_sha = compute_sha256(original);

    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    store
        .insert_run(&RunRecord {
            id: run_id.clone(),
            workflow_id: Uuid::new_v4().to_string(),
            status: "COMPLETED".to_string(),
            input_data: "{}".to_string(),
            output_data: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: Some(now.clone()),
            verified: false,
            unverified_reason: None,
        })
        .unwrap();

    let artifact = ArtifactRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        filename: "contract.pdf".to_string(),
        file_path: art_file.to_string_lossy().to_string(),
        file_size: original.len() as i64,
        mime_type: "application/pdf".to_string(),
        sha256: original_sha,
        verified: true,
        created_at: now.clone(),
    };
    store.insert_artifact(&artifact).unwrap();

    let ev = RunEvidenceRecord {
        id: Uuid::new_v4().to_string(),
        run_id: run_id.clone(),
        command: "true".to_string(),
        exit_code: 0,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: 10,
        verified: true,
        created_at: now,
        output_sha256: String::new(),
        unverified_reason: None,
    };
    store.insert_run_evidence(&ev).unwrap();

    assert!(store.mark_run_verified(&run_id, true).unwrap());
    assert!(store.is_run_verified(&run_id).unwrap());

    std::fs::write(&art_file, b"%PDF-1.4 tampered forged contract").unwrap();

    let recheck = store.mark_run_verified(&run_id, true).unwrap();
    assert!(!recheck);
    assert!(!store.is_run_verified(&run_id).unwrap());

    let run = store.get_run(&run_id).unwrap().unwrap();
    assert!(!run.verified);
    let reason = run.unverified_reason.unwrap_or_default();
    assert!(reason.contains("mismatch"));

    let tampered = store.check_artifact_tampering(ws.path(), &run_id).unwrap();
    assert!(tampered);
    assert!(!store.is_run_verified(&run_id).unwrap());
}

#[tokio::test]
async fn test_forged_exit_zero_without_artifact_records_refused() {
    let ws = tempdir().unwrap();
    let _db = tempdir().unwrap();
    let store = SqliteCheckpointStore::new(ws.path().join("forged.db")).unwrap();

    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    store
        .insert_run(&RunRecord {
            id: run_id.clone(),
            workflow_id: Uuid::new_v4().to_string(),
            status: "SUCCEEDED".to_string(),
            input_data: "{}".to_string(),
            output_data: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: Some(now.clone()),
            verified: false,
            unverified_reason: None,
        })
        .unwrap();

    store
        .insert_run_evidence(&RunEvidenceRecord {
            id: Uuid::new_v4().to_string(),
            run_id: run_id.clone(),
            command: "echo forged".to_string(),
            exit_code: 0,
            stdout: "forged".to_string(),
            stderr: String::new(),
            duration_ms: 5,
            verified: true,
            created_at: now,
            output_sha256: String::new(),
            unverified_reason: None,
        })
        .unwrap();

    let marked = store.mark_run_verified(&run_id, true).unwrap();
    assert!(!marked);

    let run = store.get_run(&run_id).unwrap().unwrap();
    assert!(!run.verified);
    assert!(run.unverified_reason.unwrap_or_default().contains("forged"));
}

#[tokio::test]
async fn test_workflow_engine_real_verification_command_e2e() {
    let ws = tempdir().unwrap();
    let db = tempdir().unwrap();
    let store = Arc::new(SqliteCheckpointStore::new(db.path().join("engine_e2e.db")).unwrap());
    let events = Arc::new(InMemoryEventSink::new());
    let policy = Arc::new(DefaultApprovalPolicy::new().with_workspace_root(ws.path()));
    let registry = ToolRegistry::new();
    let model = Arc::new(DummyModelClient);

    let run_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();
    let workspace_id = Uuid::new_v4();
    let scope = ExecutionScope::new(tenant_id, workspace_id, run_id).unwrap();

    let artifact_content = b"expected report data";
    std::fs::write(ws.path().join("report.txt"), artifact_content).unwrap();

    let now = Utc::now().to_rfc3339();
    let run_rec = RunRecord {
        id: run_id.to_string(),
        workflow_id: Uuid::new_v4().to_string(),
        status: "RUNNING".to_string(),
        input_data: "{}".to_string(),
        output_data: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        completed_at: None,
        verified: false,
        unverified_reason: None,
    };
    store.insert_run(&run_rec).unwrap();

    let spec = WorkflowSpec {
        id: Uuid::new_v4(),
        tenant_id,
        workspace_id,
        name: "Verification Engine Test".to_string(),
        version: 1,
        nodes: vec![
            WorkflowNode::Start(StartNodeSpec {
                id: "start".to_string(),
                label: "Start".to_string(),
                trigger_type: "manual".to_string(),
                input_schema: Value::Null,
                schedule: None,
            }),
            WorkflowNode::Crew(CrewNodeSpec {
                id: "crew_verify".to_string(),
                label: "Verify Crew".to_string(),
                agents: vec![AgentSpec {
                    id: "agent_tester".to_string(),
                    role: "Tester".to_string(),
                    goal: "Test verification command".to_string(),
                    backstory: "Backstory".to_string(),
                    model: "dummy".to_string(),
                    tool_ids: vec![],
                    skill_ids: vec![],
                    max_iterations: 1,
                    temperature: 0.1,
                    guardrails: vec![],
                    knowledge_capsule_ids: vec![],
                    tier: None,
                }],
                tasks: vec![TaskSpec {
                    id: "task_verify".to_string(),
                    agent_id: "agent_tester".to_string(),
                    description: "Produce report".to_string(),
                    expected_output: "Report confirmed".to_string(),
                    output_schema: "text".to_string(),
                    output_schema_json: None,
                    context_task_ids: vec![],
                    expected_artifact_paths: vec!["report.txt".to_string()],
                }],
                process: CrewProcess::Sequential,
                consensus_config: ConsensusConfigSpec::default(),
                manager_agent_id: None,
                join: JoinMode::Any,
                expected_artifact_paths: vec!["report.txt".to_string()],
                requires_visual_evidence: false,
                verification_command: Some(
                    "test -f report.txt && grep -q 'expected report data' report.txt".to_string(),
                ),
            }),
            WorkflowNode::End(EndNodeSpec {
                id: "end".to_string(),
                label: "End".to_string(),
            }),
        ],
        edges: vec![
            WorkflowEdgeSpec {
                source: "start".to_string(),
                target: "crew_verify".to_string(),
                route: None,
                loop_back: false,
            },
            WorkflowEdgeSpec {
                source: "crew_verify".to_string(),
                target: "end".to_string(),
                route: None,
                loop_back: false,
            },
        ],
        learning_edges: vec![],
        layout: BTreeMap::new(),
        modes: WorkflowModesSpec::default(),
    };

    let executor = WorkflowExecutor::new(model, store.clone(), events, policy, registry, ws.path());

    let outcome = executor.start_workflow(&spec, scope, None).await.unwrap();
    assert!(matches!(outcome, WorkflowOutcome::Completed(_)));

    let run = store.get_run(&run_id.to_string()).unwrap().unwrap();
    assert!(run.verified);
    assert!(run.unverified_reason.is_none());

    let evidence = store.list_run_evidence(&run_id.to_string()).unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].exit_code, 0);
    assert!(evidence[0].verified);
    assert!(!evidence[0].output_sha256.is_empty());

    let artifacts = store.list_artifacts(&run_id.to_string()).unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].filename, "report.txt");
    assert_eq!(artifacts[0].sha256, compute_sha256(artifact_content));
    assert!(artifacts[0].verified);
}
