pub mod ast;
pub mod document;
pub mod extractor;
pub mod html_to_ast;
pub mod markdown;

pub use ast::{Document, Block, Inline};
pub use document::*;
pub use extractor::{ContentExtractor, ExtractorConfig, ExtractionError};
pub use html_to_ast::parse_html_to_ast;
pub use markdown::{MarkdownSerializer, MarkdownConfig, HeadingStyle};
