use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use stillo_core::document::{ExtractedContent, ExtractedLink};
use url::Url;

/// HTML コンテンツを ratatui の Lines に変換して保持し、スクロールとリンク選択を管理する。
pub struct ContentView {
    pub lines: Vec<Line<'static>>,
    /// (line_index, link_index) のペア。リンクが存在する行とそのリンク番号の対応
    pub link_positions: Vec<(usize, usize)>,
    pub scroll_offset: usize,
    pub selected_link: Option<usize>,
}

impl ContentView {
    pub fn from_content(content: &ExtractedContent) -> Self {
        let mut converter = HtmlToLines::new(&content.links);
        converter.convert(&content.body_html);

        Self {
            lines: converter.lines,
            link_positions: converter.link_positions,
            scroll_offset: 0,
            selected_link: None,
        }
    }

    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }

    pub fn scroll_down(&mut self, n: usize, viewport_height: usize) {
        let max = self.lines.len().saturating_sub(viewport_height);
        self.scroll_offset = (self.scroll_offset + n).min(max);
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self, viewport_height: usize) {
        self.scroll_offset = self.lines.len().saturating_sub(viewport_height);
    }

    pub fn next_link(&mut self) {
        if self.link_positions.is_empty() {
            return;
        }
        self.selected_link = Some(match self.selected_link {
            None => 0,
            Some(i) => (i + 1).min(self.link_positions.len() - 1),
        });
        self.scroll_to_selected_link();
        self.rebuild_link_highlights();
    }

    pub fn prev_link(&mut self) {
        if self.link_positions.is_empty() {
            return;
        }
        self.selected_link = Some(match self.selected_link {
            None => 0,
            Some(i) => i.saturating_sub(1),
        });
        self.scroll_to_selected_link();
        self.rebuild_link_highlights();
    }

    pub fn selected_link_url<'a>(&self, links: &'a [ExtractedLink]) -> Option<&'a Url> {
        let sel = self.selected_link?;
        let (_, link_idx) = self.link_positions.get(sel)?;
        links.get(*link_idx).map(|l| &l.href)
    }

    /// 選択中のリンクが表示領域内に入るようスクロールする
    fn scroll_to_selected_link(&mut self) {
        if let Some(sel) = self.selected_link {
            if let Some(&(line_idx, _)) = self.link_positions.get(sel) {
                if line_idx < self.scroll_offset {
                    self.scroll_offset = line_idx;
                }
            }
        }
    }

    /// 選択状態変化後に該当行のハイライトを更新する
    fn rebuild_link_highlights(&mut self) {
        // 全リンク行を走査して selected/unselected スタイルを再適用
        for (pos_idx, &(line_idx, _)) in self.link_positions.iter().enumerate() {
            let is_selected = self.selected_link == Some(pos_idx);
            if let Some(line) = self.lines.get_mut(line_idx) {
                let style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                // 行全体のスタイルを更新
                *line = Line::styled(
                    line.spans
                        .iter()
                        .map(|s| s.content.as_ref().to_owned())
                        .collect::<Vec<_>>()
                        .join(""),
                    style,
                );
            }
        }
    }

    /// 検索クエリにマッチする行インデックスを返す
    pub fn search(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return vec![];
        }
        let q = query.to_lowercase();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                text.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

/// body_html を ratatui の Line リストに変換する状態機械
struct HtmlToLines<'a> {
    links: &'a [ExtractedLink],
    pub lines: Vec<Line<'static>>,
    pub link_positions: Vec<(usize, usize)>,
    current_spans: Vec<Span<'static>>,
    bold: bool,
    italic: bool,
    /// リンク処理中: (link_index, accumulated_text)
    link_stack: Option<(usize, String)>,
    list_depth: usize,
    link_counter: usize,
}

impl<'a> HtmlToLines<'a> {
    fn new(links: &'a [ExtractedLink]) -> Self {
        Self {
            links,
            lines: Vec::new(),
            link_positions: Vec::new(),
            current_spans: Vec::new(),
            bold: false,
            italic: false,
            link_stack: None,
            list_depth: 0,
            link_counter: 0,
        }
    }

    fn convert(&mut self, html: &str) {
        let mut pos = 0;
        let bytes = html.as_bytes();

        while pos < html.len() {
            if bytes[pos] == b'<' {
                if let Some(close) = html[pos..].find('>') {
                    let inner = &html[pos + 1..pos + close];
                    let (tag, attrs, is_closing, _) = parse_tag(inner);
                    self.handle_tag(&tag, attrs, is_closing);
                    pos += close + 1;
                    continue;
                }
            }
            let next = html[pos..].find('<').map(|i| pos + i).unwrap_or(html.len());
            let text = html_decode(&html[pos..next]);
            if !text.is_empty() {
                self.push_text(&text);
            }
            pos = next;
        }

        // 残りのスパンをフラッシュ
        self.flush_line();
    }

    fn handle_tag(&mut self, tag: &str, attrs: &str, is_closing: bool) {
        match (tag, is_closing) {
            ("h1", false) => { self.flush_line(); self.push_text("# "); self.bold = true; }
            ("h2", false) => { self.flush_line(); self.push_text("## "); self.bold = true; }
            ("h3", false) => { self.flush_line(); self.push_text("### "); self.bold = true; }
            ("h4" | "h5" | "h6", false) => { self.flush_line(); self.bold = true; }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => {
                self.bold = false;
                self.flush_line();
                self.push_empty_line();
            }
            ("p", false) => { self.flush_line(); }
            ("p", true) => { self.flush_line(); self.push_empty_line(); }
            ("br", _) => { self.flush_line(); }
            ("hr", _) => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(60),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            ("strong" | "b", false) => { self.bold = true; }
            ("strong" | "b", true) => { self.bold = false; }
            ("em" | "i", false) => { self.italic = true; }
            ("em" | "i", true) => { self.italic = false; }
            ("a", false) => {
                // リンクインデックスを attrs の href から探す
                let href = extract_attr(attrs, "href").unwrap_or_default();
                let link_idx = self.links.iter().position(|l| l.href.as_str() == href
                    || l.href.as_str().trim_end_matches('/') == href.trim_end_matches('/'));
                let idx = link_idx.unwrap_or(self.link_counter);
                self.link_stack = Some((idx, String::new()));
                self.link_counter += 1;
            }
            ("a", true) => {
                if let Some((link_idx, text)) = self.link_stack.take() {
                    let display = format!("[{}] {}", link_idx + 1, text.trim());
                    let line_idx = self.lines.len();
                    self.link_positions.push((line_idx, link_idx));
                    self.current_spans.push(Span::styled(
                        display,
                        Style::default().fg(Color::Cyan),
                    ));
                    self.flush_line();
                }
            }
            ("li", false) => {
                self.flush_line();
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                self.push_text(&format!("{}• ", indent));
            }
            ("ul" | "ol", false) => { self.list_depth += 1; }
            ("ul" | "ol", true) => {
                self.list_depth = self.list_depth.saturating_sub(1);
                self.flush_line();
            }
            ("pre", false) => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "```",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            ("pre", true) => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "```",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            ("script" | "style" | "noscript" | "iframe", _) => {}
            _ => {}
        }
    }

    fn current_style(&self) -> Style {
        let mut style = Style::default();
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        if self.italic {
            style = style.add_modifier(Modifier::ITALIC);
        }
        style
    }

    fn push_text(&mut self, text: &str) {
        if let Some((_, ref mut link_text)) = self.link_stack {
            link_text.push_str(text);
        } else if !text.is_empty() {
            let style = self.current_style();
            self.current_spans.push(Span::styled(text.to_owned(), style));
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            self.lines.push(Line::from(std::mem::take(&mut self.current_spans)));
        }
    }

    fn push_empty_line(&mut self) {
        self.lines.push(Line::from(""));
    }
}

fn parse_tag(inner: &str) -> (String, &str, bool, bool) {
    let is_self_closing = inner.ends_with('/');
    let trimmed = if is_self_closing { &inner[..inner.len() - 1] } else { inner };
    let is_closing = trimmed.starts_with('/');
    let body = if is_closing { &trimmed[1..] } else { trimmed }.trim();
    let (tag_name, attrs) = body
        .split_once(|c: char| c.is_whitespace())
        .unwrap_or((body, ""));
    (tag_name.to_lowercase(), attrs.trim(), is_closing, is_self_closing)
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    for quote in &['"', '\''] {
        let search = format!("{}={}", name, quote);
        if let Some(start_idx) = attrs.find(&search) {
            let value_start = start_idx + search.len();
            if let Some(end_offset) = attrs[value_start..].find(*quote) {
                return Some(attrs[value_start..value_start + end_offset].to_owned());
            }
        }
    }
    None
}

fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
