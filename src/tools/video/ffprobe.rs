use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::binary::find_binary;
use crate::domain::ExecutionScope;
use crate::tools::path_guard::resolve_readable_file;
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

#[derive(Clone)]
pub struct FfprobeInfoTool {
    workspace_root: PathBuf,
}

impl FfprobeInfoTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[derive(Deserialize)]
struct FfprobeArgs {
    path: String,
}

#[derive(Serialize)]
pub struct MediaInfoResult {
    pub path: String,
    pub duration_seconds: f64,
    pub duration_ms: u64,
    pub has_video: bool,
    pub has_audio: bool,
    pub video_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<u32>,
    pub audio_sample_rate: Option<u32>,
}

fn parse_frame_rate(rate_str: &str) -> Option<f64> {
    if let Some((num_str, den_str)) = rate_str.split_once('/') {
        let num: f64 = num_str.trim().parse().ok()?;
        let den: f64 = den_str.trim().parse().ok()?;
        if den > 0.0 {
            Some(num / den)
        } else {
            None
        }
    } else {
        rate_str.trim().parse().ok()
    }
}

pub async fn run_ffprobe(
    workspace_root: &Path,
    rel_path: &str,
) -> Result<MediaInfoResult, ToolError> {
    let (_real_root, real_path, canonical_rel) = resolve_readable_file(workspace_root, rel_path)?;
    let ffprobe_bin = find_binary("ffprobe").ok_or_else(|| {
        ToolError::ExecutionFailed("ffprobe binary not found on host system".to_string())
    })?;

    let output = tokio::process::Command::new(&ffprobe_bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=codec_name,codec_type,width,height,r_frame_rate,channels,sample_rate",
            "-of",
            "json",
        ])
        .arg(&real_path)
        .output()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn ffprobe: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ToolError::ExecutionFailed(format!(
            "ffprobe exited with code {:?}: {}",
            output.status.code(),
            stderr.trim()
        )));
    }

    let parsed: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        ToolError::ExecutionFailed(format!("Failed to parse ffprobe json output: {}", e))
    })?;

    let duration_seconds = parsed
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let duration_ms = (duration_seconds * 1000.0).round() as u64;

    let mut has_video = false;
    let mut has_audio = false;
    let mut video_codec = None;
    let mut width = None;
    let mut height = None;
    let mut fps = None;
    let mut audio_codec = None;
    let mut audio_channels = None;
    let mut audio_sample_rate = None;

    if let Some(streams) = parsed.get("streams").and_then(|s| s.as_array()) {
        for s in streams {
            let codec_type = s.get("codec_type").and_then(|c| c.as_str()).unwrap_or("");
            if codec_type == "video" && !has_video {
                has_video = true;
                video_codec = s
                    .get("codec_name")
                    .and_then(|c| c.as_str())
                    .map(|v| v.to_string());
                width = s.get("width").and_then(|w| w.as_u64()).map(|v| v as u32);
                height = s.get("height").and_then(|h| h.as_u64()).map(|v| v as u32);
                fps = s
                    .get("r_frame_rate")
                    .and_then(|r| r.as_str())
                    .and_then(parse_frame_rate);
            } else if codec_type == "audio" && !has_audio {
                has_audio = true;
                audio_codec = s
                    .get("codec_name")
                    .and_then(|c| c.as_str())
                    .map(|v| v.to_string());
                audio_channels = s.get("channels").and_then(|c| c.as_u64()).map(|v| v as u32);
                audio_sample_rate = s
                    .get("sample_rate")
                    .and_then(|r| r.as_str())
                    .and_then(|v| v.parse::<u32>().ok());
            }
        }
    }

    Ok(MediaInfoResult {
        path: canonical_rel,
        duration_seconds,
        duration_ms,
        has_video,
        has_audio,
        video_codec,
        width,
        height,
        fps,
        audio_codec,
        audio_channels,
        audio_sample_rate,
    })
}

impl Tool for FfprobeInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ffprobe_info".to_string(),
            description: "Extract metadata from media file: duration, fps, codec, resolution, and audio channels".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the media file in workspace"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        arguments: &'a Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: FfprobeArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid ffprobe_info arguments: {}", e))
            })?;

            let info = run_ffprobe(&self.workspace_root, &args.path).await?;
            serde_json::to_string(&info).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to serialize media info: {}", e))
            })
        })
    }
}
