use markup5ever_rcdom::{Handle, NodeData};
use url::Url;
use crate::document::{ExtractedContent, ExtractedLink, PageMetadata};

const NOISE_TAGS: &[&str] = &["nav", "header", "footer", "aside", "script", "style", "noscript", "iframe", "form"];
const NOISE_CLASS_PATTERNS: &[&str] = &["nav", "sidebar", "menu", "ad", "banner", "comment", "footer", "header", "widget"];
const CONTENT_CLASS_PATTERNS: &[&str] = &["article", "content", "main", "post", "entry", "body", "text"];

pub struct ReadabilityExtractor {
    pub preserve_links: bool,
}

impl ReadabilityExtractor {
    pub fn extract(&self, root: &Handle, base_url: &Url) -> ExtractedContent {
        let title = extract_title(root);
        let metadata = extract_metadata(root, base_url);
        let body = find_body(root);

        let main_node = body.as_ref()
            .and_then(|b| find_main_content(b))
            .or(body.clone());

        let (body_html, body_text, links) = main_node
            .as_ref()
            .map(|n| self.serialize_content(n, base_url))
            .unwrap_or_else(|| (String::new(), String::new(), Vec::new()));

        ExtractedContent {
            url: base_url.clone(),
            title: title.unwrap_or_else(|| base_url.to_string()),
            byline: metadata.og_title.clone(),
            body_text,
            body_html,
            links,
            metadata,
        }
    }

    fn serialize_content(&self, handle: &Handle, base_url: &Url) -> (String, String, Vec<ExtractedLink>) {
        let mut html = String::new();
        let mut text = String::new();
        let mut links = Vec::new();
        serialize_node(handle, &mut html, &mut text, &mut links, base_url, self.preserve_links);
        (html, text, links)
    }
}

fn find_body(root: &Handle) -> Option<Handle> {
    find_tag(root, "body")
}

fn find_tag(handle: &Handle, tag_name: &str) -> Option<Handle> {
    if let NodeData::Element { name, .. } = &handle.data {
        if name.local.as_ref() == tag_name {
            return Some(handle.clone());
        }
    }
    for child in handle.children.borrow().iter() {
        if let Some(found) = find_tag(child, tag_name) {
            return Some(found);
        }
    }
    None
}

fn find_main_content(body: &Handle) -> Option<Handle> {
    // まず <main>, <article> を優先探索
    if let Some(node) = find_tag(body, "main").or_else(|| find_tag(body, "article")) {
        return Some(node);
    }

    // スコアリングでメインコンテンツを特定
    let mut best: Option<(Handle, f64)> = None;
    score_nodes(body, &mut best);
    best.map(|(node, _)| node)
}

fn score_nodes(handle: &Handle, best: &mut Option<(Handle, f64)>) {
    if is_noise(handle) {
        return;
    }

    if let NodeData::Element { name, .. } = &handle.data {
        let tag = name.local.as_ref();
        let score = compute_score(handle, tag);
        if score > 20.0 {
            match best {
                None => *best = Some((handle.clone(), score)),
                Some((_, best_score)) if score > *best_score => {
                    *best = Some((handle.clone(), score));
                }
                _ => {}
            }
        }
    }

    for child in handle.children.borrow().iter() {
        score_nodes(child, best);
    }
}

fn compute_score(handle: &Handle, tag: &str) -> f64 {
    let base = match tag {
        "article" => 30.0,
        "section" => 10.0,
        "div" => 5.0,
        "p" => 3.0,
        "td" => 3.0,
        "blockquote" => 3.0,
        "pre" => 3.0,
        _ => 0.0,
    };

    if base == 0.0 {
        return 0.0;
    }

    // クラス/IDによる補正
    let class_bonus = class_score(handle);

    let text_len = count_text(handle) as f64;
    let link_len = count_link_text(handle) as f64;

    // リンク密度ペナルティ: リンクテキストが多いほど本文らしくない
    let link_density = if text_len > 0.0 { link_len / text_len } else { 0.0 };
    let density_penalty = link_density * 50.0;

    base + class_bonus + (text_len * 0.1).min(30.0) - density_penalty
}

fn class_score(handle: &Handle) -> f64 {
    let attrs = match &handle.data {
        NodeData::Element { attrs, .. } => attrs.borrow(),
        _ => return 0.0,
    };

    let mut score = 0.0;
    for attr in attrs.iter() {
        let name = attr.name.local.as_ref();
        if name != "class" && name != "id" {
            continue;
        }
        let val = attr.value.as_ref().to_lowercase();
        for pattern in CONTENT_CLASS_PATTERNS {
            if val.contains(pattern) {
                score += 10.0;
            }
        }
        for pattern in NOISE_CLASS_PATTERNS {
            if val.contains(pattern) {
                score -= 10.0;
            }
        }
    }
    score
}

fn is_noise(handle: &Handle) -> bool {
    match &handle.data {
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            if NOISE_TAGS.contains(&tag) {
                return true;
            }
            let attrs = attrs.borrow();
            for attr in attrs.iter() {
                let aname = attr.name.local.as_ref();
                if aname != "class" && aname != "id" {
                    continue;
                }
                let val = attr.value.as_ref().to_lowercase();
                for pattern in NOISE_CLASS_PATTERNS {
                    if val.contains(pattern) {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn count_text(handle: &Handle) -> usize {
    let mut total = 0;
    count_text_inner(handle, &mut total);
    total
}

fn count_text_inner(handle: &Handle, total: &mut usize) {
    match &handle.data {
        NodeData::Text { contents } => {
            *total += contents.borrow().trim().len();
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            if tag == "script" || tag == "style" {
                return;
            }
            for child in handle.children.borrow().iter() {
                count_text_inner(child, total);
            }
        }
        _ => {
            for child in handle.children.borrow().iter() {
                count_text_inner(child, total);
            }
        }
    }
}

fn count_link_text(handle: &Handle) -> usize {
    let mut total = 0;
    count_link_text_inner(handle, &mut total, false);
    total
}

fn count_link_text_inner(handle: &Handle, total: &mut usize, in_link: bool) {
    match &handle.data {
        NodeData::Text { contents } if in_link => {
            *total += contents.borrow().trim().len();
        }
        NodeData::Element { name, .. } => {
            let tag = name.local.as_ref();
            let is_link = tag == "a";
            for child in handle.children.borrow().iter() {
                count_link_text_inner(child, total, in_link || is_link);
            }
        }
        _ => {}
    }
}

fn serialize_node(
    handle: &Handle,
    html: &mut String,
    text: &mut String,
    links: &mut Vec<ExtractedLink>,
    base_url: &Url,
    preserve_links: bool,
) {
    if is_noise(handle) {
        return;
    }

    match &handle.data {
        NodeData::Text { contents } => {
            let t = contents.borrow();
            let trimmed = t.as_ref();
            if !trimmed.trim().is_empty() {
                html.push_str(&html_escape(trimmed));
                text.push_str(trimmed);
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            let attrs_ref = attrs.borrow();

            match tag {
                "script" | "style" | "noscript" | "iframe" => return,
                "a" if preserve_links => {
                    let href = attrs_ref.iter()
                        .find(|a| a.name.local.as_ref() == "href")
                        .map(|a| a.value.as_ref().to_owned());
                    let rel = attrs_ref.iter()
                        .find(|a| a.name.local.as_ref() == "rel")
                        .map(|a| a.value.as_ref().to_owned());

                    let resolved = href.as_deref().and_then(|h| base_url.join(h).ok());

                    html.push_str("<a");
                    if let Some(ref h) = href {
                        html.push_str(&format!(" href=\"{}\"", html_escape(h)));
                    }
                    html.push('>');

                    let mut link_text = String::new();
                    let mut link_html = String::new();
                    for child in handle.children.borrow().iter() {
                        serialize_node(child, &mut link_html, text, links, base_url, preserve_links);
                        collect_text(child, &mut link_text);
                    }
                    html.push_str(&link_html);
                    html.push_str("</a>");

                    if let Some(href_url) = resolved {
                        links.push(ExtractedLink {
                            text: link_text.trim().to_owned(),
                            href: href_url,
                            rel,
                        });
                    }
                    return;
                }
                _ => {
                    // ブロック要素
                    let is_block = matches!(tag, "p" | "div" | "section" | "article" |
                        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" |
                        "ul" | "ol" | "li" | "blockquote" | "pre" | "br" | "hr" |
                        "table" | "tr" | "td" | "th" | "thead" | "tbody");

                    if is_block {
                        html.push('<');
                        html.push_str(tag);
                        html.push('>');
                        if tag == "br" || tag == "hr" {
                            // self-closing
                        } else {
                            for child in handle.children.borrow().iter() {
                                serialize_node(child, html, text, links, base_url, preserve_links);
                            }
                            html.push_str("</");
                            html.push_str(tag);
                            html.push('>');
                        }
                    } else {
                        // インライン要素はそのまま子を出力
                        for child in handle.children.borrow().iter() {
                            serialize_node(child, html, text, links, base_url, preserve_links);
                        }
                    }
                    return;
                }
            }
        }
        _ => {}
    }
}

fn collect_text(handle: &Handle, out: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            out.push_str(contents.borrow().as_ref());
        }
        _ => {
            for child in handle.children.borrow().iter() {
                collect_text(child, out);
            }
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn extract_title(root: &Handle) -> Option<String> {
    // <title> タグを優先、次に <h1> を試みる
    if let Some(title_node) = find_tag(root, "title") {
        let mut text = String::new();
        collect_text(&title_node, &mut text);
        let trimmed = text.trim().to_owned();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if let Some(h1) = find_tag(root, "h1") {
        let mut text = String::new();
        collect_text(&h1, &mut text);
        let trimmed = text.trim().to_owned();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

fn extract_metadata(root: &Handle, base_url: &Url) -> PageMetadata {
    let mut meta = PageMetadata {
        description: None,
        og_title: None,
        og_image: None,
        canonical: None,
        published_at: None,
    };
    collect_meta(root, &mut meta, base_url);
    meta
}

fn collect_meta(handle: &Handle, meta: &mut PageMetadata, base_url: &Url) {
    if let NodeData::Element { name, attrs, .. } = &handle.data {
        let tag = name.local.as_ref();
        let attrs_ref = attrs.borrow();

        if tag == "meta" {
            let name_attr = attrs_ref.iter()
                .find(|a| a.name.local.as_ref() == "name")
                .map(|a| a.value.as_ref().to_lowercase());
            let property_attr = attrs_ref.iter()
                .find(|a| a.name.local.as_ref() == "property")
                .map(|a| a.value.as_ref().to_lowercase());
            let content = attrs_ref.iter()
                .find(|a| a.name.local.as_ref() == "content")
                .map(|a| a.value.as_ref().to_owned());

            match (name_attr.as_deref(), property_attr.as_deref(), content) {
                (Some("description"), _, Some(c)) => meta.description = Some(c),
                (_, Some("og:title"), Some(c)) => meta.og_title = Some(c),
                (_, Some("og:image"), Some(c)) => meta.og_image = Some(c),
                _ => {}
            }
        } else if tag == "link" {
            let is_canonical = attrs_ref.iter()
                .any(|a| a.name.local.as_ref() == "rel" && a.value.as_ref() == "canonical");
            if is_canonical {
                if let Some(href) = attrs_ref.iter()
                    .find(|a| a.name.local.as_ref() == "href")
                    .and_then(|a| base_url.join(a.value.as_ref()).ok())
                {
                    meta.canonical = Some(href);
                }
            }
        }
    }

    for child in handle.children.borrow().iter() {
        collect_meta(child, meta, base_url);
    }
}
