use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};
use stillo_core::document::ExtractedContent;
use url::Url;

use crate::widgets::{
    content_view::ContentView,
    link_bar::{render_hint_bar, render_input_bar},
    status_bar::render_status_bar,
};

pub enum TuiResult {
    Navigate(Url),
    Dump,
    Quit,
}

enum BrowserMode {
    Normal,
    SearchInput(String),
    UrlInput(String),
}

pub struct TuiBrowser {
    content: ExtractedContent,
    view: ContentView,
    mode: BrowserMode,
    search_matches: Vec<usize>,
    search_cursor: usize,
    history: Vec<(ExtractedContent, usize)>,
}

impl TuiBrowser {
    pub fn new(content: ExtractedContent) -> Self {
        let view = ContentView::from_content(&content);
        Self {
            content,
            view,
            mode: BrowserMode::Normal,
            search_matches: Vec::new(),
            search_cursor: 0,
            history: Vec::new(),
        }
    }

    /// CLIが履歴エントリを積む（戻る操作のため）
    pub fn push_history(&mut self, content: ExtractedContent) {
        let offset = self.view.scroll_offset;
        self.history.push((content, offset));
    }

    pub fn run(&mut self) -> Result<TuiResult> {
        terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        terminal::disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<TuiResult> {
        loop {
            let viewport_height = terminal.size()?.height.saturating_sub(2) as usize;
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                if let Some(result) = self.handle_key(key.code, key.modifiers, viewport_height) {
                    return Ok(result);
                }
            }
        }
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        viewport_height: usize,
    ) -> Option<TuiResult> {
        match &self.mode {
            BrowserMode::Normal => self.handle_normal(code, modifiers, viewport_height),
            BrowserMode::SearchInput(_) => self.handle_search_input(code),
            BrowserMode::UrlInput(_) => self.handle_url_input(code),
        }
    }

    fn handle_normal(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        viewport_height: usize,
    ) -> Option<TuiResult> {
        match code {
            // 終了
            KeyCode::Char('q') | KeyCode::Esc => return Some(TuiResult::Quit),

            // スクロール
            KeyCode::Char('j') | KeyCode::Down => self.view.scroll_down(1, viewport_height),
            KeyCode::Char('k') | KeyCode::Up => self.view.scroll_up(1),
            KeyCode::Char('d') if modifiers == KeyModifiers::CONTROL => {
                self.view.scroll_down(viewport_height / 2, viewport_height);
            }
            KeyCode::Char('u') if modifiers == KeyModifiers::CONTROL => {
                self.view.scroll_up(viewport_height / 2);
            }
            KeyCode::PageDown => self.view.scroll_down(viewport_height, viewport_height),
            KeyCode::PageUp => self.view.scroll_up(viewport_height),
            KeyCode::Char('g') | KeyCode::Home => self.view.scroll_to_top(),
            KeyCode::Char('G') | KeyCode::End => self.view.scroll_to_bottom(viewport_height),

            // リンクナビゲーション
            KeyCode::Tab => self.view.next_link(),
            KeyCode::BackTab => self.view.prev_link(),

            // リンクフォロー
            KeyCode::Enter => {
                if let Some(url) = self.view.selected_link_url(&self.content.links) {
                    return Some(TuiResult::Navigate(url.clone()));
                }
            }

            // 戻る
            KeyCode::Char('B') => {
                if let Some((prev_content, prev_offset)) = self.history.pop() {
                    let mut prev_view = ContentView::from_content(&prev_content);
                    prev_view.scroll_offset = prev_offset;
                    self.content = prev_content;
                    self.view = prev_view;
                    self.search_matches.clear();
                }
            }

            // URL入力モード
            KeyCode::Char('U') => {
                self.mode = BrowserMode::UrlInput(String::new());
            }

            // 検索モード
            KeyCode::Char('/') => {
                self.mode = BrowserMode::SearchInput(String::new());
            }

            // 次の検索マッチ
            KeyCode::Char('n') => {
                if !self.search_matches.is_empty() {
                    self.search_cursor =
                        (self.search_cursor + 1) % self.search_matches.len();
                    self.view.scroll_offset = self.search_matches[self.search_cursor];
                }
            }

            // Markdown dump
            KeyCode::Char('d') => return Some(TuiResult::Dump),

            _ => {}
        }
        None
    }

    fn handle_search_input(&mut self, code: KeyCode) -> Option<TuiResult> {
        match code {
            KeyCode::Esc => {
                self.mode = BrowserMode::Normal;
            }
            KeyCode::Enter => {
                let query = match &self.mode {
                    BrowserMode::SearchInput(q) => q.clone(),
                    _ => unreachable!(),
                };
                self.search_matches = self.view.search(&query);
                self.search_cursor = 0;
                if let Some(&line_idx) = self.search_matches.first() {
                    self.view.scroll_offset = line_idx;
                }
                self.mode = BrowserMode::Normal;
            }
            KeyCode::Backspace => {
                if let BrowserMode::SearchInput(ref mut q) = self.mode {
                    q.pop();
                }
            }
            KeyCode::Char(c) => {
                if let BrowserMode::SearchInput(ref mut q) = self.mode {
                    q.push(c);
                }
            }
            _ => {}
        }
        None
    }

    fn handle_url_input(&mut self, code: KeyCode) -> Option<TuiResult> {
        match code {
            KeyCode::Esc => {
                self.mode = BrowserMode::Normal;
            }
            KeyCode::Enter => {
                let input = match &self.mode {
                    BrowserMode::UrlInput(s) => s.clone(),
                    _ => unreachable!(),
                };
                self.mode = BrowserMode::Normal;
                if let Ok(url) = input.parse::<Url>() {
                    return Some(TuiResult::Navigate(url));
                }
                // httpスキームを補完して再試行
                if let Ok(url) = format!("https://{}", input).parse::<Url>() {
                    return Some(TuiResult::Navigate(url));
                }
            }
            KeyCode::Backspace => {
                if let BrowserMode::UrlInput(ref mut s) = self.mode {
                    s.pop();
                }
            }
            KeyCode::Char(c) => {
                if let BrowserMode::UrlInput(ref mut s) = self.mode {
                    s.push(c);
                }
            }
            _ => {}
        }
        None
    }

    fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // ステータスバー
                Constraint::Min(0),    // コンテンツ
                Constraint::Length(1), // ヒントバー / 入力バー
            ])
            .split(f.area());

        render_status_bar(f, chunks[0], &self.content.title, self.content.url.as_str());

        let viewport_height = chunks[1].height as usize;
        let visible_lines: Vec<_> = self
            .view
            .lines
            .iter()
            .skip(self.view.scroll_offset)
            .take(viewport_height)
            .cloned()
            .collect();

        let content_widget = ratatui::widgets::Paragraph::new(visible_lines)
            .style(ratatui::style::Style::default());
        f.render_widget(content_widget, chunks[1]);

        match &self.mode {
            BrowserMode::Normal => {
                render_hint_bar(
                    f,
                    chunks[2],
                    self.view.link_positions.len(),
                    self.view.selected_link,
                );
            }
            BrowserMode::SearchInput(q) => {
                render_input_bar(f, chunks[2], "/", q);
            }
            BrowserMode::UrlInput(s) => {
                render_input_bar(f, chunks[2], "URL: ", s);
            }
        }
    }
}
