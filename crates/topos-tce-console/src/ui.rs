use tui::{
    backend::Backend,
    layout::{Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Tabs},
    Frame,
};

use crate::app::App;

pub fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(5)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(size);

    let block = Block::default().style(Style::default().fg(Color::Cyan));
    f.render_widget(block, size);
    let titles = app
        .titles
        .iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(first, Style::default().fg(Color::Yellow)),
                Span::styled(rest, Style::default().fg(Color::Green)),
            ])
        })
        .collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .select(app.index)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Black),
        );
    f.render_widget(tabs, chunks[0]);
    match app.index {
        0 => {
            let events: Vec<ListItem> = app
                .network
                .iter()
                .map(|(_peer_id, peer)| {
                    let s = match peer.direction.to_lowercase().as_str() {
                        "dialer" => Style::default().fg(Color::Yellow),
                        "listener" => Style::default().fg(Color::Blue),
                        _ => Style::default(),
                    };
                    let header = Spans::from(vec![
                        Span::styled(format!("{:<9}", peer.direction), s),
                        Span::raw(" "),
                        Span::styled(
                            &peer.peer_id,
                            Style::default().add_modifier(Modifier::ITALIC),
                        ),
                    ]);

                    let log = Spans::from(Span::raw(&peer.addresses));

                    ListItem::new(vec![
                        Spans::from("-".repeat(chunks[1].width as usize)),
                        header,
                        Spans::from(""),
                        log,
                    ])
                })
                .collect();

            let inner = List::new(events)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Open connections"),
                )
                .start_corner(Corner::TopLeft);
            f.render_widget(inner, chunks[1]);
        }
        1 => {
            let events: Vec<ListItem> = app
                .certificates
                .iter()
                .map(|(_, cert)| {
                    let s = Style::default().fg(Color::Blue);

                    let header = Spans::from(vec![
                        Span::styled(format!("{:<9}", cert.subnet_id), s),
                        Span::raw(" "),
                        Span::styled(
                            &cert.cert_id,
                            Style::default().add_modifier(Modifier::ITALIC),
                        ),
                    ]);
                    ListItem::new(vec![
                        Spans::from("-".repeat(chunks[1].width as usize)),
                        header,
                        Spans::from(""),
                    ])
                })
                .collect();

            let inner = List::new(events)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Pending certificates"),
                )
                .start_corner(Corner::TopLeft);
            f.render_widget(inner, chunks[1]);
        }
        2 => {}
        3 => {
            let sample_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let keys = [
                "EchoSubscribers",
                "ReadySubscribers",
                "EchoSubscriptions",
                "ReadySubscriptions",
                "DeliverySubscriptions",
            ];
            let items: Vec<ListItem> = keys
                .iter()
                .map(|i| {
                    let lines = vec![Spans::from(i.to_string())];
                    ListItem::new(lines)
                })
                .collect();

            // Create a List from all list items and highlight the currently selected one
            let items = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Current Sample"),
                )
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">> ");

            // // We can now render the item list
            f.render_stateful_widget(items, sample_chunks[0], &mut app.current_selected_sample);

            if let Some(selected) = app.current_selected_sample.selected() {
                if let Some(key) = keys.get(selected) {
                    if let Some(items) = app.samples.get(&key.to_string()) {
                        let events: Vec<ListItem> = items
                            .iter()
                            .map(|v| {
                                let s = Style::default().fg(Color::Blue);

                                let header =
                                    Spans::from(vec![Span::styled(format!("{:<9}", v), s)]);
                                ListItem::new(vec![
                                    Spans::from("-".repeat(sample_chunks[1].width as usize)),
                                    header,
                                    Spans::from(""),
                                ])
                            })
                            .collect();

                        let inner = List::new(events)
                            .block(Block::default().borders(Borders::ALL).title("Details"))
                            .start_corner(Corner::TopLeft);

                        f.render_widget(inner, sample_chunks[1]);
                    }
                }
            }
        }
        _ => unreachable!(),
    };
}
