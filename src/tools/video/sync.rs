use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::binary::find_binary;
use crate::domain::ExecutionScope;
use crate::tools::path_guard::resolve_readable_file;
use crate::tools::types::{BoxFuture, Tool, ToolDefinition, ToolError};

pub const DEFAULT_SAMPLE_RATE: u32 = 8000;
pub const DEFAULT_MIN_CONFIDENCE: f64 = 0.50;
pub const DEFAULT_MAX_OFFSET_MS: i64 = 30000;

#[derive(Clone)]
pub struct FfmpegSyncAudioTool {
    workspace_root: PathBuf,
}

impl FfmpegSyncAudioTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[derive(Deserialize)]
struct SyncArgs {
    master_audio_path: String,
    camera_paths: Vec<String>,
    min_confidence: Option<f64>,
    max_offset_ms: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct CameraSyncResult {
    pub camera: String,
    pub offset_ms: i64,
    pub confidence: f64,
}

#[derive(Serialize)]
pub struct SyncAudioOutput {
    pub master_audio: String,
    pub results: Vec<CameraSyncResult>,
}

pub async fn extract_audio_pcm(
    ffmpeg_bin: &Path,
    file_path: &Path,
    sample_rate: u32,
) -> Result<Vec<f32>, ToolError> {
    let output = tokio::process::Command::new(ffmpeg_bin)
        .args([
            "-nostdin",
            "-v",
            "error",
            "-i",
            file_path.to_str().unwrap(),
            "-vn",
            "-ac",
            "1",
            "-ar",
            &sample_rate.to_string(),
            "-f",
            "f32le",
            "pipe:1",
        ])
        .output()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn ffmpeg: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ToolError::ExecutionFailed(format!(
            "ffmpeg audio extraction failed with exit code {:?}: {}",
            output.status.code(),
            stderr.trim()
        )));
    }

    if output.stdout.is_empty() {
        return Err(ToolError::ExecutionFailed(format!(
            "No audio samples extracted from '{}'",
            file_path.display()
        )));
    }

    let samples: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    Ok(samples)
}

pub fn compute_audio_cross_correlation(
    master: &[f32],
    camera: &[f32],
    sample_rate: u32,
    max_offset_ms: i64,
) -> (i64, f64) {
    if master.is_empty() || camera.is_empty() {
        return (0, 0.0);
    }

    let decimate = 8;
    let coarse_rate = sample_rate / decimate;
    let coarse_master: Vec<f32> = master.iter().step_by(decimate as usize).copied().collect();
    let coarse_camera: Vec<f32> = camera.iter().step_by(decimate as usize).copied().collect();

    let max_coarse_lag = ((max_offset_ms as f64 * coarse_rate as f64) / 1000.0).round() as i64;
    let m_len = coarse_master.len() as i64;
    let c_len = coarse_camera.len() as i64;

    let min_lag = -max_coarse_lag.min(c_len - 1);
    let max_lag = max_coarse_lag.min(m_len - 1);

    let mut best_coarse_lag = 0i64;
    let mut best_coarse_dot = f32::NEG_INFINITY;

    for lag in min_lag..=max_lag {
        let (m_start, c_start, len) = if lag >= 0 {
            let m_s = lag as usize;
            let c_s = 0usize;
            let available = (m_len - lag).min(c_len) as usize;
            (m_s, c_s, available)
        } else {
            let m_s = 0usize;
            let c_s = (-lag) as usize;
            let available = (c_len + lag).min(m_len) as usize;
            (m_s, c_s, available)
        };

        if len < 50 {
            continue;
        }

        let mut dot = 0.0f32;
        for i in 0..len {
            dot += coarse_master[m_start + i] * coarse_camera[c_start + i];
        }

        if dot > best_coarse_dot {
            best_coarse_dot = dot;
            best_coarse_lag = lag;
        }
    }

    let approx_sample_lag = best_coarse_lag * decimate as i64;
    let fine_window = 40i64;
    let full_m_len = master.len() as i64;
    let full_c_len = camera.len() as i64;

    let fine_min = (approx_sample_lag - fine_window).max(-full_c_len + 1);
    let fine_max = (approx_sample_lag + fine_window).min(full_m_len - 1);

    let mut best_fine_lag = approx_sample_lag;
    let mut best_confidence = 0.0f64;

    for lag in fine_min..=fine_max {
        let (m_start, c_start, len) = if lag >= 0 {
            let m_s = lag as usize;
            let c_s = 0usize;
            let available = (full_m_len - lag).min(full_c_len) as usize;
            (m_s, c_s, available)
        } else {
            let m_s = 0usize;
            let c_s = (-lag) as usize;
            let available = (full_c_len + lag).min(full_m_len) as usize;
            (m_s, c_s, available)
        };

        if len < 200 {
            continue;
        }

        let m_slice = &master[m_start..m_start + len];
        let c_slice = &camera[c_start..c_start + len];

        let mut sum_m = 0.0f64;
        let mut sum_c = 0.0f64;
        for i in 0..len {
            sum_m += m_slice[i] as f64;
            sum_c += c_slice[i] as f64;
        }
        let mean_m = sum_m / len as f64;
        let mean_c = sum_c / len as f64;

        let mut num = 0.0f64;
        let mut den_m = 0.0f64;
        let mut den_c = 0.0f64;

        for i in 0..len {
            let dm = m_slice[i] as f64 - mean_m;
            let dc = c_slice[i] as f64 - mean_c;
            num += dm * dc;
            den_m += dm * dm;
            den_c += dc * dc;
        }

        let denom = (den_m * den_c).sqrt();
        let coeff = if denom > 1e-12 {
            (num / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if coeff > best_confidence {
            best_confidence = coeff;
            best_fine_lag = lag;
        }
    }

    let offset_ms = ((best_fine_lag as f64 * 1000.0) / sample_rate as f64).round() as i64;
    (offset_ms, best_confidence)
}

impl Tool for FfmpegSyncAudioTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ffmpeg_sync_audio".to_string(),
            description: "Determine millisecond audio sync offset and confidence between camera video audio streams and a master audio reference using cross-correlation".to_string(),
            parameters_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "master_audio_path": {
                        "type": "string",
                        "description": "Relative path to master audio reference in workspace"
                    },
                    "camera_paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Relative paths to camera videos in workspace"
                    },
                    "min_confidence": {
                        "type": "number",
                        "description": "Minimum acceptable confidence threshold (default: 0.50)"
                    },
                    "max_offset_ms": {
                        "type": "integer",
                        "description": "Maximum search window in milliseconds (default: 30000)"
                    }
                },
                "required": ["master_audio_path", "camera_paths"]
            }),
        }
    }

    fn execute<'a>(
        &'a self,
        _scope: &'a ExecutionScope,
        arguments: &'a Value,
    ) -> BoxFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let args: SyncArgs = serde_json::from_value(arguments.clone()).map_err(|e| {
                ToolError::InvalidArguments(format!("Invalid ffmpeg_sync_audio arguments: {}", e))
            })?;

            if args.camera_paths.is_empty() {
                return Err(ToolError::InvalidArguments(
                    "camera_paths array cannot be empty".to_string(),
                ));
            }

            let min_conf = args.min_confidence.unwrap_or(DEFAULT_MIN_CONFIDENCE);
            let max_offset = args.max_offset_ms.unwrap_or(DEFAULT_MAX_OFFSET_MS);

            let (_master_root, master_real, master_rel) =
                resolve_readable_file(&self.workspace_root, &args.master_audio_path)?;

            let ffmpeg_bin = find_binary("ffmpeg").ok_or_else(|| {
                ToolError::ExecutionFailed("ffmpeg binary not found on host system".to_string())
            })?;

            let master_pcm =
                extract_audio_pcm(&ffmpeg_bin, &master_real, DEFAULT_SAMPLE_RATE).await?;

            let mut results = Vec::new();

            for cam_path in &args.camera_paths {
                let (_cam_root, cam_real, cam_rel) =
                    resolve_readable_file(&self.workspace_root, cam_path)?;

                let cam_pcm =
                    extract_audio_pcm(&ffmpeg_bin, &cam_real, DEFAULT_SAMPLE_RATE).await?;

                let (offset_ms, confidence) = compute_audio_cross_correlation(
                    &master_pcm,
                    &cam_pcm,
                    DEFAULT_SAMPLE_RATE,
                    max_offset,
                );

                if confidence < min_conf {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Audio sync confidence for '{}' is {:.3}, below required threshold {:.3}. Cannot determine reliable sync offset",
                        cam_rel, confidence, min_conf
                    )));
                }

                results.push(CameraSyncResult {
                    camera: cam_rel,
                    offset_ms,
                    confidence,
                });
            }

            let output = SyncAudioOutput {
                master_audio: master_rel,
                results,
            };

            serde_json::to_string(&output).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to serialize sync output: {}", e))
            })
        })
    }
}
