use std::path::Path;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use ratatui_image::StatefulImage;

use crate::{app::AppLog, viewmodel::ViewModel};

/// UIを描画
pub fn ui(f: &mut Frame, vm: &mut ViewModel) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    draw_image_panel(f, vm, main_chunks[0]);
    draw_info_panel(f, vm, main_chunks[1]);
}

/// 画像表示エリアを描画
fn draw_image_panel(f: &mut Frame, vm: &mut ViewModel, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let image_block = Block::default().title("Image").borders(Borders::ALL);
    f.render_widget(image_block, chunks[0]);

    if vm.is_fin {
        let done_block = Block::default().borders(Borders::ALL).title("Done");
        let text = Paragraph::new("All images have been sorted!")
            .style(Style::default().fg(Color::Green))
            .block(done_block)
            .alignment(Alignment::Center);
        f.render_widget(text, centered_rect(60, 20, chunks[0]));
    } else {
        let image = StatefulImage::default();
        f.render_stateful_widget(image, chunks[0], &mut vm.img);
    }

    let file_info_text = format!(
        "File: {}\nProgress: {} / {}",
        vm.img_path.display(),
        vm.progress,
        vm.img_num
    );
    let file_info_widget =
        Paragraph::new(file_info_text).block(Block::default().title("Info").borders(Borders::ALL));
    f.render_widget(file_info_widget, chunks[1]);
}

/// 情報エリアを描画
fn draw_info_panel(f: &mut Frame, vm: &ViewModel, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(0)])
        .split(area);

    // キーバインド
    let mut key_items: Vec<ListItem> = vm
        .keybind
        .iter()
        .map(|(key, folder)| {
            let text = format!("[{}] -> {}", key, folder.display());
            let style = if folder == Path::new("skip") {
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

    // ログ
    if let Some(log) = &vm.log {
        let log_widget = Paragraph::new(match log {
            AppLog::MoveSuccess(file, dest) => {
                format!("{} to {}", file.display(), dest.display())
            }
            AppLog::Skip(file) => format!("Skip {}", file.display()),
        })
        .block(Block::default().title("Last Action").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
        f.render_widget(log_widget, chunks[1]);
    }
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
