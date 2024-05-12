use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{layout::Rect, style::Stylize};

use crate::ui::{app::Action, view::Border};

use super::{UiView, View::Monitor};

pub struct PopUp<'a>(pub &'a str);

impl<'a> UiView for PopUp<'a> {
    fn handle_events(&self, event: Event) -> Option<Action> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Enter | KeyCode::Esc,
                ..
            }) => Some(Action::ChangeView(Monitor)),
            _ => None,
        }
    }

    fn render(&mut self, frame: &mut ratatui::prelude::Frame) {
        use ratatui::{
            text::Line,
            widgets::{
                block::{Position, Title},
                Clear, Paragraph, Wrap,
            },
        };
        let instructions = Title::from(Line::from(vec![
            " Press ".into(),
            "<Enter>".blue().bold(),
            " to close dialog ".into(),
        ]));

        let block = Border::NoHighlight
            .titled("Info")
            .title(instructions.position(Position::Bottom));

        let content = Paragraph::new(self.0)
            .block(block)
            .centered()
            .wrap(Wrap { trim: true });

        let area = centered_rect(60, 50, frame.size());
        frame.render_widget(Clear, area);
        frame.render_widget(content, area);
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    use ratatui::layout::{Constraint, Layout};
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
