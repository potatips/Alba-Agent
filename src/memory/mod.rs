pub mod distiller;
pub mod firewall;
pub mod vector;

pub use distiller::{
    DistillationCandidate, DistillationTrigger, EpisodicKnowledgeDistiller, MAX_HEURISTIC_LEN,
    MAX_PRECONDITIONS_LEN,
};
pub use firewall::{
    AgentLearningContext, FirewallDecision, LearningPolicy, RedactionEngine, ScopeFirewall,
};
pub use vector::{
    bytes_to_vector, capsule_searchable_text, cosine_similarity, vector_to_bytes,
    DeterministicEmbedder, VectorEmbedder, DEFAULT_VECTOR_DIMENSIONS,
};
