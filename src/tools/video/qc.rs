use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use super::binary::find_binary;
use super::ffprobe::run_ffprobe;
use crate::domain::ExecutionScope;
use crate::tools::path_guard::resolve_readable_file;
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

pub const DEFAULT_MAX_DURATION_DIFF_MS: u64 = 150;
pub const DEFAULT_MAX_BLACK_DURATION_MS: u64 = 500;
pub const DEFAULT_MAX_AV_SYNC_DIFF_MS: u64 = 100;

#[derive(Clone)]
pub struct FfmpegQcTool {
    workspace_root: PathBuf,
}

impl FfmpegQcTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[derive(Deserialize)]
struct QcArgs {
    rendered_path: String,
    expected_duration_ms: u64,
    max_duration_diff_ms: Option<u64>,
    max_black_frame_duration_ms: Option<u64>,
    max_av_sync_diff_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct QcReport {
    pub qc_passed: bool,
    pub rendered_path: String,
    pub duration_ms: u64,
    pub expected_duration_ms: u64,
    pub duration_diff_ms: u64,
    pub has_video: bool,
    pub has_audio: bool,
    pub no_long_black_frames: bool,
    pub max_detected_black_ms: u64,
    pub av_sync_ok: bool,
    pub av_sync_diff_ms: u64,
    pub failure_reasons: Vec<String>,
}

impl Tool for FfmpegQcTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ffmpeg_qc".to_string(),
            description: "Automated video quality control: verifies EDL duration match, absence of prolonged black frames, audio-video sync bounds, and non-empty streams".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "rendered_path": {
                        "type": "string",
                        "description": "Relative path to rendered video in workspace"
                    },
                    "expected_duration_ms": {
                        "type": "integer",
                        "description": "Expected EDL timeline duration in milliseconds"
                    },
                    "max_duration_diff_ms": {
                        "type": "integer",
                        "description": "Acceptable duration difference threshold in ms (default: 150)"
                    },
                    "max_black_frame_duration_ms": {
                        "type": "integer",
                        "description": "Maximum allowed continuous black frame duration in ms (default: 500)"
                    },
                    "max_av_sync_diff_ms": {
                        "type": "integer",
                        "description": "Maximum allowed audio-video duration divergence in ms (default: 100)"
                    }
                },
                "required": ["rendered_path", "expected_duration_ms"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        arguments: &'a Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: QcArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid ffmpeg_qc arguments: {}", e))
            })?;

            let max_dur_diff = args
                .max_duration_diff_ms
                .unwrap_or(DEFAULT_MAX_DURATION_DIFF_MS);
            let max_black_dur = args
                .max_black_frame_duration_ms
                .unwrap_or(DEFAULT_MAX_BLACK_DURATION_MS);
            let max_av_diff = args
                .max_av_sync_diff_ms
                .unwrap_or(DEFAULT_MAX_AV_SYNC_DIFF_MS);

            let (_r, real_path, canonical_rel) =
                resolve_readable_file(&self.workspace_root, &args.rendered_path)?;

            let info = run_ffprobe(&self.workspace_root, &args.rendered_path).await?;

            let mut failure_reasons = Vec::new();

            if !info.has_video {
                failure_reasons.push("Output is missing video stream".to_string());
            }

            if !info.has_audio {
                failure_reasons.push("Output is missing audio stream".to_string());
            }

            let duration_diff_ms =
                (info.duration_ms as i64 - args.expected_duration_ms as i64).unsigned_abs();

            if duration_diff_ms > max_dur_diff {
                failure_reasons.push(format!(
                    "Rendered duration ({} ms) deviates from expected EDL duration ({} ms) by {} ms (threshold: {} ms)",
                    info.duration_ms, args.expected_duration_ms, duration_diff_ms, max_dur_diff
                ));
            }

            let ffmpeg_bin = find_binary("ffmpeg").ok_or_else(|| {
                ToolError::ExecutionFailed("ffmpeg binary not found on host system".to_string())
            })?;

            let blackdetect_threshold = (max_black_dur as f64 / 1000.0).max(0.1);
            let black_output = tokio::process::Command::new(&ffmpeg_bin)
                .args([
                    "-nostdin",
                    "-v",
                    "info",
                    "-i",
                    real_path.to_str().unwrap(),
                    "-vf",
                    &format!("blackdetect=d={:.3}:pix_th=0.10", blackdetect_threshold),
                    "-f",
                    "null",
                    "-",
                ])
                .output()
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to run blackdetect: {}", e))
                })?;

            let black_stderr = String::from_utf8_lossy(&black_output.stderr);
            let mut max_detected_black_ms = 0u64;

            for line in black_stderr.lines() {
                if let Some(pos) = line.find("black_duration:") {
                    let sub = &line[pos + "black_duration:".len()..];
                    let dur_str = sub.split_whitespace().next().unwrap_or("0");
                    if let Ok(dur_sec) = dur_str.parse::<f64>() {
                        let dur_ms = (dur_sec * 1000.0).round() as u64;
                        if dur_ms > max_detected_black_ms {
                            max_detected_black_ms = dur_ms;
                        }
                    }
                }
            }

            let no_long_black_frames = max_detected_black_ms <= max_black_dur;
            if !no_long_black_frames {
                failure_reasons.push(format!(
                    "Prolonged black frames detected: {} ms exceeds threshold of {} ms",
                    max_detected_black_ms, max_black_dur
                ));
            }

            let ffprobe_bin = find_binary("ffprobe").ok_or_else(|| {
                ToolError::ExecutionFailed("ffprobe binary not found on host system".to_string())
            })?;

            let stream_details = tokio::process::Command::new(&ffprobe_bin)
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "stream=codec_type,duration",
                    "-of",
                    "json",
                ])
                .arg(&real_path)
                .output()
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to run stream probe: {}", e))
                })?;

            let mut video_dur_ms = info.duration_ms;
            let mut audio_dur_ms = info.duration_ms;

            if let Ok(parsed_streams) = serde_json::from_slice::<Value>(&stream_details.stdout) {
                if let Some(streams) = parsed_streams.get("streams").and_then(|s| s.as_array()) {
                    for s in streams {
                        let c_type = s.get("codec_type").and_then(|c| c.as_str()).unwrap_or("");
                        let d_sec = s
                            .get("duration")
                            .and_then(|d| d.as_str())
                            .and_then(|str_val| str_val.parse::<f64>().ok());
                        if let Some(sec) = d_sec {
                            let ms = (sec * 1000.0).round() as u64;
                            if c_type == "video" {
                                video_dur_ms = ms;
                            } else if c_type == "audio" {
                                audio_dur_ms = ms;
                            }
                        }
                    }
                }
            }

            let av_sync_diff_ms = (video_dur_ms as i64 - audio_dur_ms as i64).unsigned_abs();
            let av_sync_ok = av_sync_diff_ms <= max_av_diff;

            if !av_sync_ok {
                failure_reasons.push(format!(
                    "Audio-video stream duration mismatch: video {} ms vs audio {} ms diff {} ms (threshold: {} ms)",
                    video_dur_ms, audio_dur_ms, av_sync_diff_ms, max_av_diff
                ));
            }

            let qc_passed = failure_reasons.is_empty();

            let report = QcReport {
                qc_passed,
                rendered_path: canonical_rel,
                duration_ms: info.duration_ms,
                expected_duration_ms: args.expected_duration_ms,
                duration_diff_ms,
                has_video: info.has_video,
                has_audio: info.has_audio,
                no_long_black_frames,
                max_detected_black_ms,
                av_sync_ok,
                av_sync_diff_ms,
                failure_reasons,
            };

            serde_json::to_string(&report).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to serialize QC report: {}", e))
            })
        })
    }
}
