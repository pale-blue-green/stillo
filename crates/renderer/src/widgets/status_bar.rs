use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// 上部ステータスバー: "stillo | <title> | <url>"
pub fn render_status_bar(f: &mut Frame, area: Rect, title: &str, url: &str) {
    let text = Line::from(vec![
        Span::styled(" stillo ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(
            truncate(title, 40),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            truncate(url, area.width.saturating_sub(60) as usize),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let bar = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray));
    f.render_widget(bar, area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max || max == 0 {
        s.to_owned()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}
