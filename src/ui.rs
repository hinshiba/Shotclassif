/// UIを描画する
fn ui(f: &mut Frame, app: &mut App2, isfin: bool) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    draw_image_panel(f, app, main_chunks[0], isfin);
    draw_info_panel(f, app, main_chunks[1]);
}

/// 画像表示エリアを描画する
fn draw_image_panel(f: &mut Frame, app: &mut App2, area: Rect, isfin: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let image_block = Block::default().title("Image").borders(Borders::ALL);
    f.render_widget(image_block, chunks[0]);

    if isfin {
        let done_block = Block::default().borders(Borders::ALL).title("Done");
        let text = Paragraph::new("All images have been sorted!")
            .style(Style::default().fg(Color::Green))
            .block(done_block)
            .alignment(Alignment::Center);
        f.render_widget(text, centered_rect(60, 20, chunks[0]));
    } else if let Some(processed) = &mut app.current_img {
        let image = StatefulImage::default();
        f.render_stateful_widget(image, chunks[0], &mut processed.state);
    }

    let file_info_text = format!(
        "File: {}\nProgress: {} / {}",
        app.get_imgname().display(),
        app.processed_num,
        app.images.len()
    );
    let file_info_widget =
        Paragraph::new(file_info_text).block(Block::default().title("Info").borders(Borders::ALL));
    f.render_widget(file_info_widget, chunks[1]);
}

/// 情報エリアを描画する
fn draw_info_panel(f: &mut Frame, app: &App2, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(area);

    // キーバインド
    let mut key_items: Vec<ListItem> = app
        .config
        .dists
        .iter()
        .map(|(key, folder)| {
            let text = format!("[{}] -> {}", key, folder);
            let style = if folder.eq_ignore_ascii_case("skip") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Cyan)
            };
            ListItem::new(text).style(style)
        })
        .collect();
    key_items.push(ListItem::new("---"));
    key_items.push(ListItem::new("[q] -> exit").style(Style::default().fg(Color::Red)));

    let keys_widget = List::new(key_items)
        .block(Block::default().title("Keybinds").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(keys_widget, chunks[0]);

    // アクションログ
    let log_widget = Paragraph::new(app.last_action_message.as_str())
        .block(Block::default().title("Last Action").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(log_widget, chunks[1]);
}

/// 指定された矩形の中央に、指定されたパーセンテージの矩形を生成する
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
