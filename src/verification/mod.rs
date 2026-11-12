pub mod claims;
pub mod validator;

pub use claims::{
    clean_path_token, extract_claimed_files, looks_like_file_path, ClaimError, ClaimsHook,
};
pub use validator::{
    is_document_suffix, is_signature_ok, is_text_suffix, render_document, ArtifactCheck,
    ArtifactContractHook, ArtifactContractReport, CheckStatus,
};
