use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::fmt::Write;

/// Selects the minimum number of panels that remains readable at a terminal's
/// current width. Content widgets can render into the returned rectangles
/// without duplicating width thresholds in every screen.
pub fn columns(area: Rect) -> Vec<Rect> {
    let constraints = if area.width >= 120 {
        vec![Constraint::Percentage(30), Constraint::Percentage(45), Constraint::Percentage(25)]
    } else if area.width >= 90 {
        vec![Constraint::Percentage(62), Constraint::Percentage(38)]
    } else {
        vec![Constraint::Percentage(100)]
    };
    Layout::default().direction(Direction::Horizontal).constraints(constraints).split(area).to_vec()
}

/// Returns a bounded text window suitable for a bordered paragraph. The
/// footer keeps overflow visible instead of silently clipping important plan
/// information on small terminals.
pub fn viewport(text: &str, height: u16, requested_offset: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let capacity = usize::from(height.max(1));
    if lines.len() <= capacity {
        return text.to_owned();
    }
    let body_capacity = capacity.saturating_sub(1).max(1);
    let max_offset = lines.len().saturating_sub(body_capacity);
    let offset = requested_offset.min(max_offset);
    let remaining_after = lines.len().saturating_sub(offset + body_capacity);
    let mut visible = lines[offset..offset + body_capacity].join("\n");
    if offset > 0 {
        visible = format!("[{} lines above]\n{visible}", offset);
    }
    if remaining_after > 0 {
        let _ = write!(visible, "\n[{remaining_after} more]");
    }
    visible
}

/// Keeps a long identifier or path readable in a narrow terminal by retaining
/// both its distinguishing prefix and suffix.
pub fn ellipsize(value: &str, max_width: usize) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= max_width {
        return value.to_owned();
    }
    if max_width <= 3 {
        return characters.into_iter().take(max_width).collect();
    }
    let available = max_width - 3;
    let head = available.div_ceil(2);
    let tail = available / 2;
    let prefix = characters.iter().take(head).collect::<String>();
    let suffix = characters[characters.len() - tail..].iter().collect::<String>();
    format!("{prefix}...{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};

    #[test]
    fn responsive_columns_follow_documented_width_thresholds() {
        assert_eq!(columns(Rect::new(0, 0, 80, 24)).len(), 1);
        assert_eq!(columns(Rect::new(0, 0, 100, 30)).len(), 2);
        assert_eq!(columns(Rect::new(0, 0, 120, 30)).len(), 3);
        assert_eq!(columns(Rect::new(0, 0, 180, 45)).len(), 3);
    }

    #[test]
    fn responsive_columns_cover_the_full_width() {
        let area = Rect::new(0, 0, 180, 45);
        assert_eq!(columns(area).iter().map(|column| column.width).sum::<u16>(), area.width);
    }

    #[test]
    fn test_backend_renders_all_supported_terminal_sizes() {
        for (width, height) in [(80, 24), (100, 30), (120, 30), (180, 45)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| {
                    for (index, column) in columns(frame.area()).into_iter().enumerate() {
                        frame.render_widget(Paragraph::new(format!("panel {index}")), column);
                    }
                })
                .expect("render supported terminal size");
        }
    }

    #[test]
    fn viewport_exposes_overflow_and_respects_offset() {
        let text = "one\ntwo\nthree\nfour\nfive";
        assert_eq!(viewport(text, 3, 0), "one\ntwo\n[3 more]");
        assert_eq!(viewport(text, 3, 2), "[2 lines above]\nthree\nfour\n[1 more]");
        assert_eq!(viewport(text, 3, 99), "[3 lines above]\nfour\nfive");
    }

    #[test]
    fn ellipsize_retains_identifier_prefix_and_suffix() {
        assert_eq!(ellipsize("DESKTOP-Q17P337-very-long-device-name", 18), "DESKTOP-...ce-name");
        assert_eq!(ellipsize("abcdef", 3), "abc");
        assert_eq!(ellipsize("short", 18), "short");
    }
}
