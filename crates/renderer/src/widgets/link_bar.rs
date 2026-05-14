use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// 下部ヒントバー（通常モード）
pub fn render_hint_bar(f: &mut Frame, area: Rect, link_count: usize, selected: Option<usize>) {
    let link_info = match (link_count, selected) {
        (0, _) => String::new(),
        (n, Some(i)) => format!(" link {}/{} │", i + 1, n),
        (n, None) => format!(" {} links │", n),
    };

    let line = Line::from(vec![
        Span::styled(&link_info, Style::default().fg(Color::Yellow)),
        Span::styled(" [Enter]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("follow "),
        Span::styled("[Tab]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("next-link "),
        Span::styled("[B]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("back "),
        Span::styled("[U]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("open-url "),
        Span::styled("[/]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("search "),
        Span::styled("[d]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("dump "),
        Span::styled("[q]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw("quit"),
    ]);

    let bar = Paragraph::new(line)
        .style(Style::default().bg(Color::DarkGray));
    f.render_widget(bar, area);
}

/// 下部バー（入力モード）
pub fn render_input_bar(f: &mut Frame, area: Rect, prompt: &str, input: &str) {
    let line = Line::from(vec![
        Span::styled(prompt, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(input),
        Span::styled("█", Style::default().fg(Color::White)),
    ]);
    let bar = Paragraph::new(line)
        .style(Style::default().bg(Color::DarkGray));
    f.render_widget(bar, area);
}
