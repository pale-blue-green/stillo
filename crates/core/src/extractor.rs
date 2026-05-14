pub mod readability;
pub mod spa_detection;

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::RcDom;

use crate::document::{ExtractedContent, RawHtml, SpaDetection};
use self::readability::ReadabilityExtractor;
use self::spa_detection::{detect_spa, extract_text_length};

#[derive(Debug, Clone)]
pub struct ExtractorConfig {
    pub min_content_length: usize,
    pub noise_selectors: Vec<String>,
    pub preserve_links: bool,
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            min_content_length: 500,
            noise_selectors: vec![],
            preserve_links: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    #[error("Failed to decode HTML: {0}")]
    Decode(String),
    #[error("No content found")]
    NoContent,
}

pub struct ContentExtractor {
    config: ExtractorConfig,
}

impl ContentExtractor {
    pub fn new(config: ExtractorConfig) -> Self {
        Self { config }
    }

    /// RawHtml → ExtractedContent（純粋関数）
    pub fn extract(&self, raw: &RawHtml) -> Result<ExtractedContent, ExtractionError> {
        let html_str = self.decode_bytes(raw)?;
        let dom = self.parse_html(&html_str);
        let root = dom.document.clone();

        let text_len = extract_text_length(&root);
        let _spa = detect_spa(&root, text_len, self.config.min_content_length);

        let extractor = ReadabilityExtractor {
            preserve_links: self.config.preserve_links,
        };
        let content = extractor.extract(&root, &raw.url);

        Ok(content)
    }

    pub fn detect_spa_for(&self, raw: &RawHtml) -> Result<SpaDetection, ExtractionError> {
        let html_str = self.decode_bytes(raw)?;
        let dom = self.parse_html(&html_str);
        let root = dom.document.clone();
        let text_len = extract_text_length(&root);
        Ok(detect_spa(&root, text_len, self.config.min_content_length))
    }

    fn decode_bytes(&self, raw: &RawHtml) -> Result<String, ExtractionError> {
        // content-type から文字コードを判定、デフォルトは UTF-8
        // BOMチェックを行い、フォールバックはlatin1として再解釈
        if let Ok(s) = std::str::from_utf8(&raw.bytes) {
            return Ok(s.to_owned());
        }
        // latin1 フォールバック
        Ok(raw.bytes.iter().map(|&b| b as char).collect())
    }

    fn parse_html(&self, html: &str) -> RcDom {
        parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .read_from(&mut html.as_bytes())
            .unwrap_or_default()
    }
}
