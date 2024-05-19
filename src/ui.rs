use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols,
    widgets::{
        block::Title, Axis, Block, Borders, Chart, Clear, Dataset, GraphType, LegendPosition, Row,
        Table, Widget,
    },
    Frame,
};

use crate::app;

const PALETTE_DARK: &[Color] = &[
    Color::Indexed(202),
    Color::Indexed(10),
    Color::Indexed(11),
    Color::Indexed(13),
    Color::Indexed(14),
    Color::Indexed(27),
    Color::Indexed(40),
    Color::Indexed(57),
    Color::Indexed(174),
    Color::Indexed(244),
];

impl Widget for &app::App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut max_values = HashMap::new();
        let (max_name_len, min, max) = self
            .signals()
            .map(|(name, set)| {
                let min_max = set.iter().fold((f64::MAX, f64::MIN), |acc, &(_, val)| {
                    (acc.0.min(val), acc.1.max(val))
                });
                (name, min_max)
            })
            .fold((0, f64::MAX, f64::MIN), |acc, (name, (min, max))| {
                let val = max_values.entry(name).or_insert(f64::MIN);
                *val = val.max(max);

                (acc.0.max(name.len()), acc.1.min(min), acc.2.max(max))
            });

        let datasets: Vec<Dataset> = self
            .signals()
            .enumerate()
            .map(|(idx, (name, set))| {
                let name = format!(
                    "{name:0$} {1:.2} (max {2:.2})",
                    max_name_len,
                    set.last().unwrap_or(&(0.0, f64::NAN)).1,
                    max_values.get(name).unwrap_or(&f64::NAN),
                );
                Dataset::default()
                    .name(name)
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(PALETTE_DARK[idx % PALETTE_DARK.len()]))
                    .data(set)
            })
            .collect();

        let last = self.elapsed();
        let window_width = [last - self.window.as_secs_f64(), last];
        let mut x_axis = Axis::default()
            .style(Style::default().fg(Color::Gray))
            .bounds(window_width);
        let window_height = [min, max];
        let mut y_axis = Axis::default()
            .style(Style::default().fg(Color::Gray))
            // .labels(vec!["-20".bold(), "0".into(), "20".bold()])
            .bounds(window_height);

        let mut legend_position = None;
        if self.legend {
            legend_position = Some(LegendPosition::TopLeft);
            x_axis = x_axis.title(format!("w={:.2?} h={:.2?}", self.window, self.history));
        }
        if self.axis_labels {
            x_axis = x_axis.labels(vec![
                format!("{:.1?}", self.window).into(),
                format!("{:.1?}", self.window / 2).into(),
                "0s ago".into(),
            ]);

            y_axis = y_axis.labels(vec![
                format!("{:.2}", window_height[0]).into(),
                format!("{:.2}", window_height.iter().sum::<f64>() / 2.0).into(),
                format!("{:.2}", window_height[1]).into(),
            ]);
        }

        let chart = Chart::new(datasets)
            .legend_position(legend_position)
            .hidden_legend_constraints((Constraint::Min(0), Constraint::Min(0)))
            .x_axis(x_axis)
            .y_axis(y_axis);

        chart.render(area, buf);
    }
}

pub fn render_help(f: &mut Frame) {
    let title = Title::from(" Help ".bold());
    let popup_block = Block::default()
        .title(title.alignment(Alignment::Center))
        .borders(Borders::ALL)
        .style(Style::default());

    let area = centered_rect(60, 40, f.size());
    let rows = [
        Row::new(vec!["q", "quit"]),
        Row::new(vec!["?", "show/hide help"]),
        Row::new(vec!["w", "norrow the chart data window by 20%"]),
        Row::new(vec!["W", "expand the chart data window by 20%"]),
        Row::new(vec!["h", "keep 2x less history"]),
        Row::new(vec!["H", "keep 2x more history"]),
        Row::new(vec!["a", "show/hide axis labels"]),
        Row::new(vec!["l", "show/hide legend"]),
    ];
    // Columns widths are constrained in the same way as Layout...
    let widths = Constraint::from_fills([3, 18]);
    let table = Table::new(rows, widths)
        // ...and they can be separated by a fixed spacing.
        .column_spacing(1)
        // You can set the style of the entire Table.
        // .style(Style::new().blue())
        // It has an optional header, which is simply a Row always visible at the top.
        .header(
            Row::new(vec!["Key", "Action"])
                .style(Style::new().bold())
                // To add space between the header and the rest of the rows, specify the margin
                .bottom_margin(1),
        )
        // As any other widget, a Table can be wrapped in a Block.
        .block(popup_block)
        // The selected row and its content can also be styled.
        .highlight_style(Style::new().reversed())
        // ...and potentially show a symbol in front of the selection.
        .highlight_symbol(">>");

    f.render_widget(Clear, area);
    f.render_widget(table, area)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
