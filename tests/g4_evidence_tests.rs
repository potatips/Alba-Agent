use std::sync::Arc;

use axum::http::StatusCode;
use chrono::Utc;
use nexus_albaai::checkpoints::sqlite::SqliteCheckpointStore;
use nexus_albaai::checkpoints::types::{RunEvidenceRecord, RunRecord};
use nexus_albaai::server::create_app;
use nexus_albaai::server::state::AppState;
use serde_json::Value;
use tempfile::tempdir;
use uuid::Uuid;

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
    };
    store.insert_run(&run_rec).unwrap();

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
    };
    store.insert_run(&run_rec).unwrap();

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
    };

    store.insert_run_evidence(&ev1).unwrap();
    store.insert_run_evidence(&ev2).unwrap();

    let list = store.list_run_evidence(&run_id).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].command, "cargo check");
    assert_eq!(list[1].command, "cargo test");
    assert_eq!(list[0].duration_ms, 210);
    assert_eq!(list[1].duration_ms, 780);
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
    };
    store.insert_run(&run_rec).unwrap();

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
}
