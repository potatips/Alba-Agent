use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

use nexus_albaai::domain::ExecutionScope;
use nexus_albaai::tools::video::{
    find_binary, FfmpegQcTool, FfmpegRenderTool, FfmpegSyncAudioTool, FfprobeInfoTool,
};
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

fn create_synthetic_audio(output_path: &Path, duration_secs: u32, freq: u32) -> bool {
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
            &format!("sine=duration={}:frequency={}", duration_secs, freq),
            "-c:a",
            "pcm_s16le",
            output_path.to_str().unwrap(),
        ])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

fn create_burst_audio(output_path: &Path, delay_ms: u32, duration_secs: u32) -> bool {
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
                "sine=frequency=600:duration=0.3,adelay={}|{}",
                delay_ms, delay_ms
            ),
            "-t",
            &duration_secs.to_string(),
            "-c:a",
            "pcm_s16le",
            output_path.to_str().unwrap(),
        ])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

fn create_burst_video_clip(
    output_path: &Path,
    delay_ms: u32,
    duration_secs: u32,
    width: u32,
    height: u32,
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
                "testsrc=duration={}:size={}x{}:rate=30",
                duration_secs, width, height
            ),
            "-f",
            "lavfi",
            "-i",
            &format!(
                "sine=frequency=600:duration=0.3,adelay={}|{}",
                delay_ms, delay_ms
            ),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-t",
            &duration_secs.to_string(),
            output_path.to_str().unwrap(),
        ])
        .output();

    match status {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
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

#[tokio::test]
async fn test_ffmpeg_sync_audio_detects_known_offsets() {
    if !is_tool_available("ffmpeg") {
        eprintln!("SKIPPED: ffmpeg not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let master = dir.path().join("master.wav");
    let cam1 = dir.path().join("cam1.mp4");
    let cam2 = dir.path().join("cam2.mp4");

    assert!(create_burst_audio(&master, 1000, 4));
    assert!(create_burst_video_clip(&cam1, 1500, 4, 320, 240));
    assert!(create_burst_video_clip(&cam2, 2200, 4, 320, 240));

    let tool = FfmpegSyncAudioTool::new(dir.path());
    let scope = default_scope();

    let res = tool
        .execute(
            &scope,
            &serde_json::json!({
                "master_audio_path": "master.wav",
                "camera_paths": ["cam1.mp4", "cam2.mp4"],
                "min_confidence": 0.50
            }),
        )
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&res).unwrap();
    let results = parsed["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);

    let offset1 = results[0]["offset_ms"].as_i64().unwrap();
    let conf1 = results[0]["confidence"].as_f64().unwrap();
    assert!((offset1 + 500).abs() < 50, "offset1 was {}", offset1);
    assert!(conf1 >= 0.70, "conf1 was {}", conf1);

    let offset2 = results[1]["offset_ms"].as_i64().unwrap();
    let conf2 = results[1]["confidence"].as_f64().unwrap();
    assert!((offset2 + 1200).abs() < 50, "offset2 was {}", offset2);
    assert!(conf2 >= 0.70, "conf2 was {}", conf2);
}

#[tokio::test]
async fn test_ffmpeg_sync_audio_rejects_low_confidence() {
    if !is_tool_available("ffmpeg") {
        eprintln!("SKIPPED: ffmpeg not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let master = dir.path().join("master.wav");
    let cam = dir.path().join("cam_diff.mp4");

    assert!(create_burst_audio(&master, 1000, 4));
    let ffmpeg = find_binary("ffmpeg").unwrap();
    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=4:size=320x240:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=3500:duration=4",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            cam.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(status.status.success());

    let tool = FfmpegSyncAudioTool::new(dir.path());
    let scope = default_scope();

    let err = tool
        .execute(
            &scope,
            &serde_json::json!({
                "master_audio_path": "master.wav",
                "camera_paths": ["cam_diff.mp4"],
                "min_confidence": 0.80
            }),
        )
        .await
        .unwrap_err();

    assert!(format!("{}", err).contains("Audio sync confidence"));
}

#[tokio::test]
async fn test_ffmpeg_sync_audio_rejects_path_traversal() {
    let dir = tempdir().unwrap();
    let tool = FfmpegSyncAudioTool::new(dir.path());
    let scope = default_scope();

    let err = tool
        .execute(
            &scope,
            &serde_json::json!({
                "master_audio_path": "../secret.wav",
                "camera_paths": ["cam1.mp4"]
            }),
        )
        .await
        .unwrap_err();

    assert!(format!("{}", err).contains("Parent traversal"));
}

#[tokio::test]
async fn test_ffmpeg_render_edl_duration_and_qc_pass() {
    if !is_tool_available("ffmpeg") || !is_tool_available("ffprobe") {
        eprintln!("SKIPPED: ffmpeg or ffprobe not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let cam1 = dir.path().join("cam1.mp4");
    let cam2 = dir.path().join("cam2.mp4");
    let master = dir.path().join("master.wav");

    assert!(create_synthetic_clip(&cam1, 5, 440, 320, 240, 30));
    assert!(create_synthetic_clip(&cam2, 5, 880, 320, 240, 30));
    assert!(create_synthetic_audio(&master, 5, 440));

    let render_tool = FfmpegRenderTool::new(dir.path());
    let scope = default_scope();

    let render_res = render_tool
        .execute(
            &scope,
            &serde_json::json!({
                "output_path": "final_render.mp4",
                "edl": {
                    "cuts": [
                        {
                            "camera": "cam1.mp4",
                            "source_start_ms": 1000,
                            "source_end_ms": 3000,
                            "timeline_start_ms": 0,
                            "timeline_end_ms": 2000
                        },
                        {
                            "camera": "cam2.mp4",
                            "source_start_ms": 2000,
                            "source_end_ms": 4000,
                            "timeline_start_ms": 2000,
                            "timeline_end_ms": 4000
                        }
                    ],
                    "master_audio_path": "master.wav"
                }
            }),
        )
        .await
        .unwrap();

    let render_val: serde_json::Value = serde_json::from_str(&render_res).unwrap();
    assert_eq!(render_val["duration_ms"], 4000);
    assert!(render_val["size_bytes"].as_u64().unwrap() > 0);

    let qc_tool = FfmpegQcTool::new(dir.path());
    let qc_res = qc_tool
        .execute(
            &scope,
            &serde_json::json!({
                "rendered_path": "final_render.mp4",
                "expected_duration_ms": 4000
            }),
        )
        .await
        .unwrap();

    let qc_val: serde_json::Value = serde_json::from_str(&qc_res).unwrap();
    assert_eq!(qc_val["qc_passed"], true, "QC report: {}", qc_res);
    assert_eq!(qc_val["has_video"], true);
    assert_eq!(qc_val["has_audio"], true);
    assert_eq!(qc_val["no_long_black_frames"], true);
    assert_eq!(qc_val["av_sync_ok"], true);
    assert!(qc_val["failure_reasons"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_ffmpeg_render_rejects_invalid_edl() {
    if !is_tool_available("ffmpeg") || !is_tool_available("ffprobe") {
        eprintln!("SKIPPED: ffmpeg or ffprobe not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let cam1 = dir.path().join("cam1.mp4");
    assert!(create_synthetic_clip(&cam1, 3, 440, 320, 240, 30));

    let render_tool = FfmpegRenderTool::new(dir.path());
    let scope = default_scope();

    let err_exceed = render_tool
        .execute(
            &scope,
            &serde_json::json!({
                "output_path": "out1.mp4",
                "edl": {
                    "cuts": [
                        {
                            "camera": "cam1.mp4",
                            "source_start_ms": 0,
                            "source_end_ms": 10000,
                            "timeline_start_ms": 0,
                            "timeline_end_ms": 10000
                        }
                    ]
                }
            }),
        )
        .await
        .unwrap_err();
    assert!(format!("{}", err_exceed).contains("exceeds camera clip duration"));

    let err_overlap = render_tool
        .execute(
            &scope,
            &serde_json::json!({
                "output_path": "out2.mp4",
                "edl": {
                    "cuts": [
                        {
                            "camera": "cam1.mp4",
                            "source_start_ms": 0,
                            "source_end_ms": 2000,
                            "timeline_start_ms": 0,
                            "timeline_end_ms": 2000
                        },
                        {
                            "camera": "cam1.mp4",
                            "source_start_ms": 1000,
                            "source_end_ms": 2500,
                            "timeline_start_ms": 1500,
                            "timeline_end_ms": 3000
                        }
                    ]
                }
            }),
        )
        .await
        .unwrap_err();
    assert!(format!("{}", err_overlap).contains("Overlapping cuts on timeline"));

    let err_missing = render_tool
        .execute(
            &scope,
            &serde_json::json!({
                "output_path": "out3.mp4",
                "edl": {
                    "cuts": [
                        {
                            "camera": "ghost_cam.mp4",
                            "source_start_ms": 0,
                            "source_end_ms": 1000,
                            "timeline_start_ms": 0,
                            "timeline_end_ms": 1000
                        }
                    ]
                }
            }),
        )
        .await
        .unwrap_err();
    assert!(format!("{}", err_missing).contains("does not exist"));
}

#[tokio::test]
async fn test_ffmpeg_render_enforces_size_and_path_limits() {
    if !is_tool_available("ffmpeg") || !is_tool_available("ffprobe") {
        eprintln!("SKIPPED: ffmpeg or ffprobe not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let cam1 = dir.path().join("cam1.mp4");
    assert!(create_synthetic_clip(&cam1, 3, 440, 320, 240, 30));

    let render_tool = FfmpegRenderTool::new(dir.path());
    let scope = default_scope();

    let err_traversal = render_tool
        .execute(
            &scope,
            &serde_json::json!({
                "output_path": "../escape.mp4",
                "edl": {
                    "cuts": [
                        {
                            "camera": "cam1.mp4",
                            "source_start_ms": 0,
                            "source_end_ms": 1000,
                            "timeline_start_ms": 0,
                            "timeline_end_ms": 1000
                        }
                    ]
                }
            }),
        )
        .await
        .unwrap_err();
    assert!(format!("{}", err_traversal).contains("Parent traversal"));

    let err_size = render_tool
        .execute(
            &scope,
            &serde_json::json!({
                "output_path": "tiny_limit.mp4",
                "max_output_bytes": 100,
                "edl": {
                    "cuts": [
                        {
                            "camera": "cam1.mp4",
                            "source_start_ms": 0,
                            "source_end_ms": 1000,
                            "timeline_start_ms": 0,
                            "timeline_end_ms": 1000
                        }
                    ]
                }
            }),
        )
        .await
        .unwrap_err();
    assert!(format!("{}", err_size).contains("exceeds limit"));
    assert!(!dir.path().join("tiny_limit.mp4").exists());
}

#[tokio::test]
async fn test_ffmpeg_qc_detects_black_frames() {
    if !is_tool_available("ffmpeg") || !is_tool_available("ffprobe") {
        eprintln!("SKIPPED: ffmpeg or ffprobe not installed on host");
        return;
    }

    let dir = tempdir().unwrap();
    let black_clip = dir.path().join("black_clip.mp4");
    let ffmpeg = find_binary("ffmpeg").unwrap();

    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:duration=2:size=320x240:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=2:frequency=440",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            black_clip.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(status.status.success());

    let qc_tool = FfmpegQcTool::new(dir.path());
    let scope = default_scope();

    let qc_res = qc_tool
        .execute(
            &scope,
            &serde_json::json!({
                "rendered_path": "black_clip.mp4",
                "expected_duration_ms": 2000,
                "max_black_frame_duration_ms": 300
            }),
        )
        .await
        .unwrap();

    let qc_val: serde_json::Value = serde_json::from_str(&qc_res).unwrap();
    assert_eq!(qc_val["qc_passed"], false);
    assert_eq!(qc_val["no_long_black_frames"], false);
    let reasons = qc_val["failure_reasons"].as_array().unwrap();
    assert!(reasons
        .iter()
        .any(|r| r.as_str().unwrap().contains("black frames")));
}
