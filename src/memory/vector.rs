use sha2::{Digest, Sha256};

use crate::checkpoints::types::KnowledgeCapsuleRecord;

pub const DEFAULT_VECTOR_DIMENSIONS: usize = 64;

pub trait VectorEmbedder: Send + Sync {
    fn embed(&self, text: &str) -> Vec<f32>;
    fn dimensions(&self) -> usize;
}

#[derive(Debug, Clone)]
pub struct DeterministicEmbedder {
    dimensions: usize,
}

impl Default for DeterministicEmbedder {
    fn default() -> Self {
        Self::new(DEFAULT_VECTOR_DIMENSIONS)
    }
}

impl DeterministicEmbedder {
    pub fn new(dimensions: usize) -> Self {
        assert!(dimensions >= 4, "dimensions must be >= 4");
        Self { dimensions }
    }
}

impl VectorEmbedder for DeterministicEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dimensions];
        for token in text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.')
        {
            let t = token.trim().to_lowercase();
            if t.is_empty() {
                continue;
            }
            let hash = Sha256::digest(t.as_bytes());
            let val = u64::from_be_bytes(hash[0..8].try_into().expect("hash slice is 8 bytes"));
            let bucket = (val as usize) % self.dimensions;
            if (val & 1) == 0 {
                vec[bucket] += 1.0;
            } else {
                vec[bucket] -= 1.0;
            }
        }

        let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vec.iter_mut() {
                *v /= norm;
            }
        }
        vec
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom > 0.0 {
        (dot / denom).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

pub fn vector_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

pub fn bytes_to_vector(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err(format!(
            "invalid byte slice length for f32 vector: {}",
            bytes.len()
        ));
    }
    let (chunks, _) = bytes.as_chunks::<4>();
    let mut vec = Vec::with_capacity(chunks.len());
    for &arr in chunks {
        vec.push(f32::from_le_bytes(arr));
    }
    Ok(vec)
}

pub fn capsule_searchable_text(capsule: &KnowledgeCapsuleRecord) -> String {
    let tags = capsule.source_domain_tags.join(" ");
    format!(
        "{} {} {} {}",
        capsule.source_role, tags, capsule.trigger_preconditions, capsule.distilled_heuristic
    )
}
