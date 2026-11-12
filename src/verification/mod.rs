pub mod validator;

pub use validator::{
    is_document_suffix, is_signature_ok, is_text_suffix, render_document, ArtifactCheck,
    ArtifactContractHook, ArtifactContractReport, CheckStatus,
};
