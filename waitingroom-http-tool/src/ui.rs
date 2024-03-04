use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{palette::tailwind, Color, Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{
        Block, BorderType, Borders, Cell, HighlightSpacing, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, Table,
    },
    Frame,
};

use crate::app::App;

pub const ITEM_HEIGHT: usize = 1;

const INFO_TEXT: &str =
    "(Esc) quit | (↑) move up | (↓) move down | (a) add a user | (d) remove the current user";

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_style_fg: color.c400,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
            footer_border_color: color.c400,
        }
    }
}

/// Renders the user interface widgets.
pub fn render(app: &mut App, frame: &mut Frame) {
    let rectangles = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).split(frame.size());
    render_table(app, frame, rectangles[0]);

    render_scrollbar(frame, app, rectangles[0]);

    render_footer(frame, app, rectangles[1]);
}

fn render_table(app: &mut App, frame: &mut Frame, area: Rect) {
    let colors = TableColors::new(&tailwind::BLUE);

    let header_style = Style::default().fg(colors.header_fg).bg(colors.header_bg);
    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(colors.selected_style_fg);

    let header = ["Status", "Next refresh time", "Queue position"]
        .iter()
        .cloned()
        .map(Cell::from)
        .collect::<Row>()
        .style(header_style)
        .height(1);
    let rows = app.users.iter().enumerate().map(|(i, data)| {
        let color = match i % 2 {
            0 => colors.normal_row_color,
            _ => colors.alt_row_color,
        };
        fn option_to_string<T: std::fmt::Display>(o: &Option<T>) -> String {
            match o {
                Some(v) => v.to_string(),
                None => "".to_string(),
            }
        }
        [
            data.status.to_string(),
            option_to_string(&data.next_refresh),
            option_to_string(&data.queue_position),
        ]
        .iter()
        .map(|content| Cell::from(Text::from(content.to_string())))
        .collect::<Row>()
        .style(Style::new().fg(colors.row_fg).bg(color))
        .height(ITEM_HEIGHT as u16)
    });
    let bar = " █ ";
    let t = Table::new(
        rows,
        [
            // + 1 is for padding.
            Constraint::Length(16 + 1),
            Constraint::Length(20 + 1),
            Constraint::Min(4),
        ],
    )
    .header(header)
    .highlight_style(selected_style)
    .highlight_symbol(Text::from(vec![
        "".into(),
        bar.into(),
        bar.into(),
        "".into(),
    ]))
    .bg(colors.buffer_bg)
    .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(t, area, &mut app.table_state);
}

fn render_scrollbar(frame: &mut Frame, app: &mut App, area: Rect) {
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None),
        area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        }),
        &mut app.scroll_state,
    );
}

fn render_footer(f: &mut Frame, _app: &mut App, area: Rect) {
    let colors = TableColors::new(&tailwind::BLUE);
    let info_footer = Paragraph::new(Line::from(INFO_TEXT))
        .style(Style::new().fg(colors.row_fg).bg(colors.buffer_bg))
        .centered()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(colors.footer_border_color))
                .border_type(BorderType::Double),
        );
    f.render_widget(info_footer, area);
}
