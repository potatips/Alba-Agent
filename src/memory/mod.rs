pub mod compaction;
pub mod distiller;
pub mod firewall;
pub mod transfer;
pub mod vector;

pub use compaction::{
    compact_conversation_history, format_deterministic_summary, group_messages,
    verify_tool_call_integrity, CompactionTrace, MessageGroup,
};
pub use distiller::{
    DistillationCandidate, DistillationTrigger, EpisodicKnowledgeDistiller, MAX_HEURISTIC_LEN,
    MAX_PRECONDITIONS_LEN,
};
pub use firewall::{
    AgentLearningContext, FirewallDecision, LearningPolicy, RedactionEngine, ScopeFirewall,
};
pub use transfer::{
    compute_flip_rates, quarantine_if_negative, score_transfer_pair, DetectorConfig, FlipRates,
    TraceTransition, TransferImpactScore, TransferObservation, TransferPairDecision,
};
pub use vector::{
    bytes_to_vector, capsule_searchable_text, cosine_similarity, vector_to_bytes,
    DeterministicEmbedder, VectorEmbedder, DEFAULT_VECTOR_DIMENSIONS,
};
