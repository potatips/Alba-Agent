use std::collections::HashMap;
use std::fs;

use nexus_albaai::verification::validator::{ArtifactContractHook, CheckStatus};

#[test]
fn test_artifact_exists_and_verified() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let file_path = ws.join("report.txt");
    fs::write(&file_path, "sample valid content\n").expect("write");

    let hook = ArtifactContractHook::new();
    let check = hook.validate_file(ws, "report.txt");

    assert_eq!(check.status, CheckStatus::Verified);
    assert_eq!(check.path, "report.txt");
    assert!(check.size_bytes > 0);
    assert!(check.reason.is_empty());
}

#[test]
fn test_artifact_missing_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let hook = ArtifactContractHook::new();
    let check = hook.validate_file(ws, "missing_deliverable.md");

    assert_eq!(check.status, CheckStatus::Missing);
    assert!(check.reason.contains("without writing this file"));
}

#[test]
fn test_artifact_empty_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let file_path = ws.join("empty.txt");
    fs::write(&file_path, b"").expect("write");

    let hook = ArtifactContractHook::new();
    let check = hook.validate_file(ws, "empty.txt");

    assert_eq!(check.status, CheckStatus::Invalid);
    assert_eq!(check.reason, "The file is empty");
    assert_eq!(check.size_bytes, 0);
}

#[test]
fn test_artifact_wrong_signature_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let fake_png = ws.join("image.png");
    fs::write(&fake_png, b"NOT_A_REAL_PNG_HEADER_HERE").expect("write");

    let fake_pdf = ws.join("document.pdf");
    fs::write(&fake_pdf, b"NOT_A_REAL_PDF_HEADER_HERE").expect("write");

    let fake_zip = ws.join("archive.zip");
    fs::write(&fake_zip, b"NOT_A_REAL_ZIP_HEADER_HERE").expect("write");

    let real_png = ws.join("real.png");
    fs::write(
        &real_png,
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01",
    )
    .expect("write");

    let hook = ArtifactContractHook::new();

    let check_png = hook.validate_file(ws, "image.png");
    assert_eq!(check_png.status, CheckStatus::Invalid);
    assert!(check_png.reason.contains("not a valid .png container"));

    let check_pdf = hook.validate_file(ws, "document.pdf");
    assert_eq!(check_pdf.status, CheckStatus::Invalid);
    assert!(check_pdf.reason.contains("not a valid .pdf container"));

    let check_zip = hook.validate_file(ws, "archive.zip");
    assert_eq!(check_zip.status, CheckStatus::Invalid);
    assert!(check_zip.reason.contains("not a valid .zip container"));

    let check_real_png = hook.validate_file(ws, "real.png");
    assert_eq!(check_real_png.status, CheckStatus::Verified);
}

#[test]
fn test_artifact_json_validation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let valid_json = ws.join("valid.json");
    fs::write(&valid_json, r#"{"status": "ok", "count": 42}"#).expect("write");

    let invalid_json = ws.join("invalid.json");
    fs::write(&invalid_json, r#"{"status": "ok", truncated"#).expect("write");

    let hook = ArtifactContractHook::new();

    let check_valid = hook.validate_file(ws, "valid.json");
    assert_eq!(check_valid.status, CheckStatus::Verified);

    let check_invalid = hook.validate_file(ws, "invalid.json");
    assert_eq!(check_invalid.status, CheckStatus::Invalid);
    assert_eq!(check_invalid.reason, "The file is not valid JSON");
}

#[test]
fn test_artifact_whitespace_text_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let whitespace_file = ws.join("blank.md");
    fs::write(&whitespace_file, "   \n\t  \n  ").expect("write");

    let hook = ArtifactContractHook::new();
    let check = hook.validate_file(ws, "blank.md");

    assert_eq!(check.status, CheckStatus::Invalid);
    assert_eq!(check.reason, "The file contains only whitespace");
}

#[test]
fn test_artifact_nested_paths_validation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let nested_dir = ws.join("outputs").join("sub").join("nested");
    fs::create_dir_all(&nested_dir).expect("create_dir_all");

    let nested_file = nested_dir.join("result.txt");
    fs::write(&nested_file, "nested content verified").expect("write");

    let hook = ArtifactContractHook::new();
    let check = hook.validate_file(ws, "outputs/sub/nested/result.txt");

    assert_eq!(check.status, CheckStatus::Verified);
    assert_eq!(check.path, "outputs/sub/nested/result.txt");
}

#[test]
fn test_artifact_symlink_escape_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let outside_dir = tempfile::tempdir().expect("outside tempdir");
    let secret_outside = outside_dir.path().join("secret.txt");
    fs::write(&secret_outside, "sensitive secret data").expect("write");

    let symlink_path = ws.join("escaped_symlink.txt");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&secret_outside, &symlink_path).expect("symlink");

    let hook = ArtifactContractHook::new();
    let check = hook.validate_file(ws, "escaped_symlink.txt");

    assert_eq!(check.status, CheckStatus::Invalid);
    assert!(check.reason.contains("Symlink escapes workspace root"));
}

#[test]
fn test_artifact_traversal_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let hook = ArtifactContractHook::new();

    let check_traversal = hook.validate_file(ws, "../outside.txt");
    assert_eq!(check_traversal.status, CheckStatus::Invalid);

    let check_abs = hook.validate_file(ws, "/etc/passwd");
    assert_eq!(check_abs.status, CheckStatus::Invalid);

    let check_null = hook.validate_file(ws, "file\0.txt");
    assert_eq!(check_null.status, CheckStatus::Invalid);
}

#[test]
fn test_artifact_structured_output_recovery() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let mut map = serde_json::Map::new();
    map.insert(
        "summary".to_string(),
        serde_json::Value::String("Completed research successfully".to_string()),
    );
    map.insert(
        "deliverables".to_string(),
        serde_json::json!(["doc1", "doc2"]),
    );

    let hook = ArtifactContractHook::new();
    let report = hook.validate_node_artifacts(
        ws,
        "node_1",
        &["docs/final_report.md".to_string()],
        Some(&map),
        None,
    );

    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].status, CheckStatus::Recovered);
    assert!(report.passed());

    let written = fs::read_to_string(ws.join("docs/final_report.md")).expect("read recovered");
    assert!(written.contains("# Final Report"));
    assert!(written.contains("Completed research successfully"));
}

#[test]
fn test_artifact_sources_materialization() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ws = temp.path();

    let mut sources = HashMap::new();
    sources.insert(
        "generated/exported.txt".to_string(),
        b"data from computer sandbox export".to_vec(),
    );

    let hook = ArtifactContractHook::new();
    let report = hook.validate_node_artifacts(
        ws,
        "node_2",
        &["generated/exported.txt".to_string()],
        None,
        Some(&sources),
    );

    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].status, CheckStatus::Verified);
    assert!(report.passed());

    let on_disk = fs::read_to_string(ws.join("generated/exported.txt")).expect("read exported");
    assert_eq!(on_disk, "data from computer sandbox export");
}
