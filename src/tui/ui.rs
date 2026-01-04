use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use super::state::AppState;

pub fn render(frame: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Status bar
            Constraint::Length(6),  // Active streams
            Constraint::Min(8),     // Request log
            Constraint::Length(3),  // Statistics
            Constraint::Length(3),  // Voices
            Constraint::Length(1),  // Help
        ])
        .split(frame.area());

    render_status_bar(frame, state, chunks[0]);
    render_active_streams(frame, state, chunks[1]);
    render_request_log(frame, state, chunks[2]);
    render_statistics(frame, state, chunks[3]);
    render_voices(frame, state, chunks[4]);
    render_help(frame, chunks[5]);
}

fn render_status_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    let uptime = state.uptime();
    let hours = uptime.as_secs() / 3600;
    let minutes = (uptime.as_secs() % 3600) / 60;
    let seconds = uptime.as_secs() % 60;

    let status_text = Line::from(vec![
        Span::raw("  Status: "),
        Span::styled("● Running", Style::default().fg(Color::Green).bold()),
        Span::raw("    Address: "),
        Span::styled(&state.listen_addr, Style::default().fg(Color::Cyan)),
        Span::raw("    Uptime: "),
        Span::styled(
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds),
            Style::default().fg(Color::Yellow),
        ),
    ]);

    let block = Block::default()
        .title(" VibeVoice TTS Provider ")
        .title_style(Style::default().bold().fg(Color::Magenta))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(status_text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_active_streams(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title(format!(" Active Streams ({}) ", state.active_streams.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    if state.active_streams.is_empty() {
        let paragraph = Paragraph::new("  No active streams")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let stream_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); state.active_streams.len().min(4)])
        .split(inner);

    for (i, stream) in state.active_streams.values().take(4).enumerate() {
        let elapsed = stream.started_at.elapsed().as_secs_f32();

        let line = Line::from(vec![
            Span::styled(
                format!("  #{:<2}", stream.id),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{:<12}", truncate(&stream.voice_id, 12)),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(" \""),
            Span::styled(
                truncate(&stream.text_preview, 30),
                Style::default().fg(Color::White),
            ),
            Span::raw("\"  "),
            Span::styled(
                format!("{:>3} chunks", stream.chunks_sent),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:.1}s", elapsed),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, stream_areas[i]);
    }
}

fn render_request_log(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .title(" Recent Requests ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    if state.request_log.is_empty() {
        let paragraph = Paragraph::new("  No requests yet")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let header = Row::new(vec!["", "Time", "Voice", "Text", "Duration", "Chunks"])
        .style(Style::default().fg(Color::DarkGray))
        .height(1);

    let rows: Vec<Row> = state
        .request_log
        .iter()
        .take(area.height.saturating_sub(4) as usize)
        .map(|log| {
            let elapsed = log.timestamp.elapsed();
            let time_ago = if elapsed.as_secs() < 60 {
                format!("{}s ago", elapsed.as_secs())
            } else {
                format!("{}m ago", elapsed.as_secs() / 60)
            };

            let (status, status_style) = if log.success {
                ("✓", Style::default().fg(Color::Green))
            } else {
                ("✗", Style::default().fg(Color::Red))
            };

            let duration = log
                .duration
                .map(|d| format!("{:.1}s", d.as_secs_f32()))
                .unwrap_or_else(|| "--".to_string());

            let chunks = log
                .chunks
                .map(|c| format!("{}", c))
                .unwrap_or_else(|| log.error.clone().unwrap_or_else(|| "--".to_string()));

            Row::new(vec![
                Span::styled(status, status_style),
                Span::styled(time_ago, Style::default().fg(Color::DarkGray)),
                Span::styled(truncate(&log.voice_id, 12), Style::default().fg(Color::Cyan)),
                Span::raw(truncate(&log.text_preview, 30)),
                Span::styled(duration, Style::default().fg(Color::Yellow)),
                Span::raw(chunks),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(block);

    frame.render_widget(table, area);
}

fn render_statistics(frame: &mut Frame, state: &AppState, area: Rect) {
    let stats = &state.stats;
    let success_rate = if stats.total_requests > 0 {
        (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0
    } else {
        100.0
    };

    let audio_mins = stats.total_audio_duration.as_secs_f64() / 60.0;

    let text = Line::from(vec![
        Span::raw("  Requests: "),
        Span::styled(
            format!("{}", stats.total_requests),
            Style::default().fg(Color::White).bold(),
        ),
        Span::raw("    Success: "),
        Span::styled(
            format!("{} ({:.1}%)", stats.successful_requests, success_rate),
            Style::default().fg(Color::Green),
        ),
        Span::raw("    Errors: "),
        Span::styled(
            format!("{}", stats.failed_requests),
            Style::default().fg(if stats.failed_requests > 0 {
                Color::Red
            } else {
                Color::DarkGray
            }),
        ),
        Span::raw("    Audio: "),
        Span::styled(
            format!("{:.1} min", audio_mins),
            Style::default().fg(Color::Cyan),
        ),
    ]);

    let block = Block::default()
        .title(" Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_voices(frame: &mut Frame, state: &AppState, area: Rect) {
    let voices_text: Vec<Span> = state
        .voices
        .iter()
        .map(|v| {
            let is_default = state.default_voice.as_ref() == Some(&v.id);
            if is_default {
                Span::styled(
                    format!("{} (default)", v.display_name),
                    Style::default().fg(Color::Cyan).bold(),
                )
            } else {
                Span::styled(v.display_name.clone(), Style::default().fg(Color::White))
            }
        })
        .collect::<Vec<_>>();

    let mut line_spans = vec![Span::raw("  ")];
    for (i, span) in voices_text.into_iter().enumerate() {
        if i > 0 {
            line_spans.push(Span::raw("    "));
        }
        line_spans.push(span);
    }

    let block = Block::default()
        .title(" Voices ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    let paragraph = Paragraph::new(Line::from(line_spans)).block(block);
    frame.render_widget(paragraph, area);
}

fn render_help(frame: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::raw("  "),
        Span::styled("[Q]", Style::default().fg(Color::Yellow).bold()),
        Span::raw("uit  "),
        Span::styled("[C]", Style::default().fg(Color::Yellow).bold()),
        Span::raw("lear log"),
    ]);

    let paragraph = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}
