use serde::{Deserialize, Serialize};
use std::path::Path;

use super::ffprobe::run_ffprobe;
use crate::tools::path_guard::resolve_readable_file;
use crate::tools::types::ToolError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdlCut {
    pub camera: String,
    pub source_start_ms: u64,
    pub source_end_ms: u64,
    pub timeline_start_ms: u64,
    pub timeline_end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditDecisionList {
    pub cuts: Vec<EdlCut>,
    pub master_audio_path: Option<String>,
}

pub async fn validate_edl(workspace_root: &Path, edl: &EditDecisionList) -> Result<u64, ToolError> {
    if edl.cuts.is_empty() {
        return Err(ToolError::InvalidArguments(
            "EDL cuts array cannot be empty".to_string(),
        ));
    }

    let mut sorted_cuts = edl.cuts.clone();
    sorted_cuts.sort_by_key(|c| c.timeline_start_ms);

    if sorted_cuts[0].timeline_start_ms != 0 {
        return Err(ToolError::InvalidArguments(format!(
            "EDL timeline must start at 0 ms, found {}",
            sorted_cuts[0].timeline_start_ms
        )));
    }

    for (idx, cut) in sorted_cuts.iter().enumerate() {
        if cut.source_end_ms <= cut.source_start_ms {
            return Err(ToolError::InvalidArguments(format!(
                "Cut {} has invalid source range: start {} ms >= end {} ms",
                idx, cut.source_start_ms, cut.source_end_ms
            )));
        }

        if cut.timeline_end_ms <= cut.timeline_start_ms {
            return Err(ToolError::InvalidArguments(format!(
                "Cut {} has invalid timeline range: start {} ms >= end {} ms",
                idx, cut.timeline_start_ms, cut.timeline_end_ms
            )));
        }

        let source_duration = cut.source_end_ms - cut.source_start_ms;
        let timeline_duration = cut.timeline_end_ms - cut.timeline_start_ms;
        if source_duration != timeline_duration {
            return Err(ToolError::InvalidArguments(format!(
                "Cut {} duration mismatch: source duration {} ms != timeline duration {} ms",
                idx, source_duration, timeline_duration
            )));
        }

        let (_real_root, _real_path, _canonical_rel) =
            resolve_readable_file(workspace_root, &cut.camera)?;

        let media_info = run_ffprobe(workspace_root, &cut.camera).await?;
        let clip_dur_ms = media_info.duration_ms;
        let tolerance_ms = 150u64;
        if cut.source_end_ms > clip_dur_ms + tolerance_ms {
            return Err(ToolError::InvalidArguments(format!(
                "Cut {} source_end_ms ({} ms) exceeds camera clip duration ({} ms) for '{}'",
                idx, cut.source_end_ms, clip_dur_ms, cut.camera
            )));
        }

        if idx > 0 {
            let prev = &sorted_cuts[idx - 1];
            if cut.timeline_start_ms < prev.timeline_end_ms {
                return Err(ToolError::InvalidArguments(format!(
                    "Overlapping cuts on timeline: cut {} ends at {} ms but cut {} starts at {} ms",
                    idx - 1,
                    prev.timeline_end_ms,
                    idx,
                    cut.timeline_start_ms
                )));
            }
        }
    }

    if let Some(ref audio_path) = edl.master_audio_path {
        let (_root, _real, _rel) = resolve_readable_file(workspace_root, audio_path)?;
    }

    let total_timeline_ms = sorted_cuts.last().map(|c| c.timeline_end_ms).unwrap_or(0);

    Ok(total_timeline_ms)
}
