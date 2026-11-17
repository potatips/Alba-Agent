use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Semaphore;

use super::binary::find_binary;
use super::edl::{validate_edl, EditDecisionList};
use crate::domain::ExecutionScope;
use crate::tools::path_guard::{resolve_readable_file, resolve_writable_file};
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

pub const DEFAULT_MAX_RENDER_SECONDS: u64 = 300;
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 500 * 1024 * 1024;
pub const DEFAULT_CONCURRENT_RENDERS: usize = 4;

#[derive(Clone)]
pub struct FfmpegRenderTool {
    workspace_root: PathBuf,
    default_max_seconds: u64,
    default_max_bytes: usize,
    semaphore: Arc<Semaphore>,
}

impl FfmpegRenderTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            default_max_seconds: DEFAULT_MAX_RENDER_SECONDS,
            default_max_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            semaphore: Arc::new(Semaphore::new(DEFAULT_CONCURRENT_RENDERS)),
        }
    }

    pub fn with_limits(
        mut self,
        max_seconds: u64,
        max_bytes: usize,
        concurrent_limit: usize,
    ) -> Self {
        self.default_max_seconds = max_seconds;
        self.default_max_bytes = max_bytes;
        self.semaphore = Arc::new(Semaphore::new(concurrent_limit));
        self
    }
}

#[derive(Deserialize)]
struct RenderArgs {
    output_path: String,
    edl: EditDecisionList,
    max_render_seconds: Option<u64>,
    max_output_bytes: Option<usize>,
}

#[derive(Serialize)]
pub struct RenderOutput {
    pub output_path: String,
    pub duration_ms: u64,
    pub size_bytes: usize,
    pub cuts_count: usize,
}

impl Tool for FfmpegRenderTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ffmpeg_render".to_string(),
            description: "Render multi-camera video from an Edit Decision List (EDL) using ffmpeg"
                .to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "output_path": {
                        "type": "string",
                        "description": "Relative output path for rendered MP4 video in workspace"
                    },
                    "edl": {
                        "type": "object",
                        "description": "Edit Decision List containing cuts array and optional master_audio_path"
                    },
                    "max_render_seconds": {
                        "type": "integer",
                        "description": "Timeout in seconds for render process (default: 300)"
                    },
                    "max_output_bytes": {
                        "type": "integer",
                        "description": "Maximum allowed output file size in bytes (default: 500MB)"
                    }
                },
                "required": ["output_path", "edl"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        arguments: &'a Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: RenderArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid ffmpeg_render arguments: {}", e))
            })?;

            let max_seconds = args.max_render_seconds.unwrap_or(self.default_max_seconds);
            let max_bytes = args.max_output_bytes.unwrap_or(self.default_max_bytes);

            let (_out_root, real_output, rel_output) =
                resolve_writable_file(&self.workspace_root, &args.output_path)?;

            if let Some(parent) = real_output.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to create output directory: {}", e))
                })?;
            }

            let total_duration_ms = validate_edl(&self.workspace_root, &args.edl).await?;

            let ffmpeg_bin = find_binary("ffmpeg").ok_or_else(|| {
                ToolError::ExecutionFailed("ffmpeg binary not found on host system".to_string())
            })?;

            let _permit = self.semaphore.acquire().await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to acquire render semaphore: {}", e))
            })?;

            let mut cmd = tokio::process::Command::new(&ffmpeg_bin);
            cmd.arg("-nostdin").arg("-y");

            let mut filter_complex = String::new();

            for (idx, cut) in args.edl.cuts.iter().enumerate() {
                let (_r, real_cam, _rel) =
                    resolve_readable_file(&self.workspace_root, &cut.camera)?;
                cmd.arg("-i").arg(real_cam);

                let start_sec = cut.source_start_ms as f64 / 1000.0;
                let end_sec = cut.source_end_ms as f64 / 1000.0;

                filter_complex.push_str(&format!(
                    "[{}:v]trim=start={:.3}:end={:.3},setpts=PTS-STARTPTS[v{}];",
                    idx, start_sec, end_sec, idx
                ));
            }

            let num_cuts = args.edl.cuts.len();
            for i in 0..num_cuts {
                filter_complex.push_str(&format!("[v{}]", i));
            }
            filter_complex.push_str(&format!("concat=n={}:v=1:a=0[outv]", num_cuts));

            let has_master_audio = args.edl.master_audio_path.is_some();
            let audio_input_idx = num_cuts;

            if let Some(ref audio_path) = args.edl.master_audio_path {
                let (_r, real_audio, _rel) =
                    resolve_readable_file(&self.workspace_root, audio_path)?;
                cmd.arg("-i").arg(real_audio);
            }

            cmd.arg("-filter_complex").arg(&filter_complex);
            cmd.arg("-map").arg("[outv]");

            if has_master_audio {
                cmd.arg("-map").arg(format!("{}:a", audio_input_idx));
            }

            let total_seconds = total_duration_ms as f64 / 1000.0;
            cmd.args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-t",
                &format!("{:.3}", total_seconds),
            ]);
            cmd.arg(&real_output);

            let execution_future = cmd.output();
            let timeout_duration = Duration::from_secs(max_seconds);

            let output = match tokio::time::timeout(timeout_duration, execution_future).await {
                Ok(res) => res.map_err(|e| {
                    ToolError::ExecutionFailed(format!("ffmpeg process failed to run: {}", e))
                })?,
                Err(_) => {
                    if real_output.exists() {
                        tokio::fs::remove_file(&real_output).await.map_err(|e| {
                            ToolError::ExecutionFailed(format!(
                                "Failed to remove partial output on timeout: {}",
                                e
                            ))
                        })?;
                    }
                    return Err(ToolError::ExecutionFailed(format!(
                        "ffmpeg render timed out after {} seconds",
                        max_seconds
                    )));
                }
            };

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if real_output.exists() {
                    tokio::fs::remove_file(&real_output).await.map_err(|e| {
                        ToolError::ExecutionFailed(format!(
                            "Failed to clean up output after ffmpeg failure: {}",
                            e
                        ))
                    })?;
                }
                return Err(ToolError::ExecutionFailed(format!(
                    "ffmpeg render exited with code {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                )));
            }

            let metadata = tokio::fs::metadata(&real_output).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to stat rendered output: {}", e))
            })?;

            let file_size = metadata.len() as usize;
            if file_size > max_bytes {
                tokio::fs::remove_file(&real_output).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to clean up oversized output file: {}",
                        e
                    ))
                })?;
                return Err(ToolError::FileSizeExceeded {
                    size: file_size,
                    limit: max_bytes,
                });
            }

            let result = RenderOutput {
                output_path: rel_output,
                duration_ms: total_duration_ms,
                size_bytes: file_size,
                cuts_count: num_cuts,
            };

            serde_json::to_string(&result).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to serialize render result: {}", e))
            })
        })
    }
}
