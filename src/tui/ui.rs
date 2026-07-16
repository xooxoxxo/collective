use crate::entry::Danger;
use crate::tui::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // filter
            Constraint::Min(6),    // table
            Constraint::Length(9), // detail
            Constraint::Length(1), // help
        ])
        .split(f.area());

    // filter box
    let title = format!("collective — {} entries", app.all.len());
    let filter = Paragraph::new(format!("filter> {}", app.filter))
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(filter, chunks[0]);

    // table
    let visible = app.visible();
    let rows = visible.iter().enumerate().map(|(i, e)| {
        let star = if app.favorites.contains(&e.id) { "★" } else { " " };
        let danger = format!("{:?}", e.danger).to_lowercase();
        let style = if i == app.selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else if e.danger == Danger::High {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(star),
            Cell::from(e.id.clone()),
            Cell::from(e.title.clone()),
            Cell::from(danger),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(30),
            Constraint::Min(20),
            Constraint::Length(7),
        ],
    )
    .header(Row::new(vec!["", "id", "title", "danger"]).style(Style::default().add_modifier(Modifier::BOLD)))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(table, chunks[1]);

    // detail pane
    let detail = match app.selected_entry() {
        Some(e) => {
            let mut lines = vec![
                Line::from(Span::styled(e.title.clone(), Style::default().add_modifier(Modifier::BOLD))),
                Line::from(format!("cmd:  {}", e.cmd)),
            ];
            if let Some(u) = e.undo.as_deref().filter(|u| !u.is_empty()) {
                lines.push(Line::from(format!("undo: {u}")));
            }
            lines.push(Line::from(format!(
                "domains: {}   danger: {:?}",
                e.domains.join(", "),
                e.danger
            )));
            lines.push(Line::from(e.explanation.trim().to_string()));
            lines.push(Line::from(format!("source: {}", e.source)));
            Paragraph::new(lines).wrap(Wrap { trim: true })
        }
        None => Paragraph::new("no match"),
    }
    .block(Block::default().borders(Borders::ALL).title("detail"));
    f.render_widget(detail, chunks[2]);

    // help bar
    let help = Paragraph::new("↵ prefill  y copy  f ★  F fav-only  type to filter  q quit")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, chunks[3]);
}
