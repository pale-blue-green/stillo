pub mod readability;
pub mod spa_detection;

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};
use url::Url;

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
        let html_str = decode_bytes(raw);
        let dom = parse_html(&html_str);
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
        let html_str = decode_bytes(raw);
        let dom = parse_html(&html_str);
        let root = dom.document.clone();
        let text_len = extract_text_length(&root);
        Ok(detect_spa(&root, text_len, self.config.min_content_length))
    }

    /// frameset ページのフレーム URL 一覧を返す。空なら通常ページ。
    pub fn detect_frames(&self, raw: &RawHtml) -> Vec<Url> {
        let html_str = decode_bytes(raw);
        let dom = parse_html(&html_str);
        collect_frame_srcs(&dom.document, &raw.url)
    }
}

/// HTTP Content-Type ヘッダーと HTML meta charset を参照してエンコードを検出し UTF-8 に変換する。
/// 判定できない場合は UTF-8 → latin1 の順でフォールバック。
fn decode_bytes(raw: &RawHtml) -> String {
    let charset = extract_charset_from_content_type(&raw.content_type)
        .or_else(|| sniff_charset_from_bytes(&raw.bytes));

    if let Some(label) = charset {
        if let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes()) {
            let (cow, _, _) = enc.decode(&raw.bytes);
            return cow.into_owned();
        }
    }

    if let Ok(s) = std::str::from_utf8(&raw.bytes) {
        return s.to_owned();
    }

    // latin1 フォールバック（文字化けは甘受する）
    raw.bytes.iter().map(|&b| b as char).collect()
}

fn parse_html(html: &str) -> RcDom {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_default()
}

/// "text/html; charset=Shift_JIS" → Some("Shift_JIS")
fn extract_charset_from_content_type(content_type: &str) -> Option<String> {
    for part in content_type.split(';') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("charset=") {
            return Some(val.trim_matches('"').to_owned());
        }
    }
    None
}

/// HTML バイト列の先頭 4096 バイトをバイト列のまま走査し、
/// `charset=` 属性値を ASCII レベルで抽出する。
/// Shift_JIS 等の非 UTF-8 バイト列でも meta タグ部分は ASCII のため動作する。
fn sniff_charset_from_bytes(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(4096)];
    let needle = b"charset=";
    let pos = head.windows(needle.len()).position(|w| {
        w.eq_ignore_ascii_case(needle)
    })?;
    let after = &head[pos + needle.len()..];
    // 引用符を読み飛ばす
    let after = after.strip_prefix(b"\"").or_else(|| after.strip_prefix(b"'")).unwrap_or(after);
    let val: Vec<u8> = after
        .iter()
        .copied()
        .take_while(|&b| !matches!(b, b'"' | b'\'' | b';' | b' ' | b'>' | b'\n' | b'\r'))
        .collect();
    if val.is_empty() {
        return None;
    }
    String::from_utf8(val).ok().filter(|s| !s.is_empty())
}

/// DOM 中の <frame src="..."> / <iframe src="..."> の URL を収集する。
fn collect_frame_srcs(handle: &markup5ever_rcdom::Handle, base: &Url) -> Vec<Url> {
    let mut result = Vec::new();
    collect_frame_srcs_inner(handle, base, &mut result);
    result
}

fn collect_frame_srcs_inner(
    handle: &markup5ever_rcdom::Handle,
    base: &Url,
    out: &mut Vec<Url>,
) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag = name.local.as_ref();
        if tag == "frame" || tag == "iframe" {
            if let Some(src) = attrs
                .borrow()
                .iter()
                .find(|a| a.name.local.as_ref() == "src")
                .map(|a| a.value.as_ref().to_owned())
            {
                if let Ok(url) = base.join(&src) {
                    out.push(url);
                }
            }
        }
    }
    for child in handle.children.borrow().iter() {
        collect_frame_srcs_inner(child, base, out);
    }
}
