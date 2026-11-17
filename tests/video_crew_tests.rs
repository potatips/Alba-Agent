use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::tools::video::{find_binary, FfprobeInfoTool};
use nexus_albaai::tools::Tool;
use tempfile::tempdir;
use uuid::Uuid;

fn default_scope() -> ExecutionScope {
    ExecutionScope {
        tenant_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    }
}

fn is_tool_available(name: &str) -> bool {
    find_binary(name).is_some()
}

fn create_synthetic_clip(
    output_path: &Path,
    duration_secs: u32,
    freq: u32,
    width: u32,
    height: u32,
    fps: u32,
) -> bool {
    let ffmpeg = match find_binary("ffmpeg") {
        Some(bin) => bin,
        None => return false,
    };

    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!(
                "testsrc=duration={}:size={}x{}:rate={}",
                duration_secs, width, height, fps
            ),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=duration={}:frequency={}", duration_secs, freq),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            output_path.to_str().unwrap(),
        ])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

#[tokio::test]
async fn test_ffprobe_info_on_synthetic_clip() {
    if !is_tool_available("ffmpeg") || !is_tool_available("ffprobe") {
        eprintln!("SKIPPED: ffmpeg or ffprobe not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let clip_path = dir.path().join("clip1.mp4");
    assert!(create_synthetic_clip(&clip_path, 2, 440, 320, 240, 30));

    let tool = FfprobeInfoTool::new(dir.path());
    let scope = default_scope();
    let res = tool
        .execute(
            &scope,
            &serde_json::json!({
                "path": "clip1.mp4"
            }),
        )
        .await
        .unwrap();

    let val: serde_json::Value = serde_json::from_str(&res).unwrap();
    assert_eq!(val["has_video"], true);
    assert_eq!(val["has_audio"], true);
    assert_eq!(val["width"], 320);
    assert_eq!(val["height"], 240);
    assert_eq!(val["fps"], 30.0);
    assert_eq!(val["audio_channels"], 1);
    let dur = val["duration_seconds"].as_f64().unwrap();
    assert!((dur - 2.0).abs() < 0.2);
}

#[tokio::test]
async fn test_ffprobe_info_rejects_path_traversal() {
    let dir = tempdir().unwrap();
    let tool = FfprobeInfoTool::new(dir.path());
    let scope = default_scope();

    let err = tool
        .execute(
            &scope,
            &serde_json::json!({
                "path": "../secret.mp4"
            }),
        )
        .await
        .unwrap_err();

    assert!(format!("{}", err).contains("Parent traversal"));
}

#[tokio::test]
async fn test_ffprobe_info_rejects_absolute_path() {
    let dir = tempdir().unwrap();
    let tool = FfprobeInfoTool::new(dir.path());
    let scope = default_scope();

    let err = tool
        .execute(
            &scope,
            &serde_json::json!({
                "path": "/tmp/outside.mp4"
            }),
        )
        .await
        .unwrap_err();

    assert!(format!("{}", err).contains("Absolute paths are forbidden"));
}

#[tokio::test]
async fn test_ffprobe_info_rejects_symlink_escape() {
    let outside_dir = tempdir().unwrap();
    let workspace = tempdir().unwrap();

    let outside_file = outside_dir.path().join("outside.mp4");
    fs::write(&outside_file, b"fake video content").unwrap();

    let link_path = workspace.path().join("link_to_outside.mp4");
    symlink(&outside_file, &link_path).unwrap();

    let tool = FfprobeInfoTool::new(workspace.path());
    let scope = default_scope();

    let err = tool
        .execute(
            &scope,
            &serde_json::json!({
                "path": "link_to_outside.mp4"
            }),
        )
        .await
        .unwrap_err();

    assert!(format!("{}", err).contains("escapes workspace root"));
}

#[tokio::test]
async fn test_ffprobe_info_not_found() {
    let dir = tempdir().unwrap();
    let tool = FfprobeInfoTool::new(dir.path());
    let scope = default_scope();

    let err = tool
        .execute(
            &scope,
            &serde_json::json!({
                "path": "non_existent.mp4"
            }),
        )
        .await
        .unwrap_err();

    assert!(
        format!("{}", err).contains("not found") || format!("{}", err).contains("does not exist")
    );
}
