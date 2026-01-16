//! Markdown processing with syntax highlighting and transformations

pub mod encrypted_blocks;
mod parser;
mod pipeline;
pub mod transforms;

pub use encrypted_blocks::{
    extract_encrypted_blocks, extract_html_encrypted_blocks, replace_placeholders,
};
pub use pipeline::{Pipeline, TransformContext};
