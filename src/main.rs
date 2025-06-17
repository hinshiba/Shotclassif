use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::ImageReader;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self},
    io::{self, Stdout},
    path::{Path, PathBuf},
    time::Duration,
};

/// TOML file structure
#[derive(Deserialize, Debug)]
struct Config {
    dir: String,
    dists: HashMap<char, String>,
}

struct App {
    images: Vec<PathBuf>,
    imgstate: Option<StatefulProtocol>,
    idx: usize,
    config: Config,
    should_quit: bool,
    last_action_message: String,
}

impl App {
    fn new(config: Config) -> Result<Self> {
        // 画像形式
        let img_extensions = ["jpg", "jpeg", "png", "gif", "bmp"];
        let dir = Path::new(&config.dir);
        if !dir.exists() {
            return Err(anyhow::anyhow!("dir is not valid: {}", config.dir));
        }

        let images: Vec<PathBuf> = fs::read_dir(dir)
            .with_context(|| format!("cannot read dir: {}", config.dir))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map_or(false, |ext| {
                            img_extensions.contains(&ext.to_lowercase().as_str())
                        })
            })
            .collect();

        if images.is_empty() {
            return Err(anyhow::anyhow!("dir is empty: {}", config.dir));
        }

        Ok(Self {
            images,
            imgstate: None,
            idx: 0,
            config,
            should_quit: false,
            last_action_message: String::new(),
        })
    }

    fn on_key(&mut self, key: char) -> Result<()> {
        if let Some(dist) = self.config.dists.get(&key) {
            // "skip" は特別扱い
            if dist == "skip" {
                self.last_action_message =
                    format!("skip: {}", self.get_imgname().unwrap_or_default());
                self.next_image();
                return Ok(());
            }

            self.move_current_image(&dist.clone())?;
            self.next_image();
        }
        Ok(())
    }

    fn move_current_image(&mut self, dist: &str) -> Result<()> {
        if self.is_finished() {
            return Ok(());
        }

        let now_imgpath = &self.images[self.idx];
        let file_name = now_imgpath.file_name().context("failed get file neme")?;

        let dist = Path::new(dist);
        fs::create_dir_all(dist)
            .with_context(|| format!("dist create failed: {}", dist.display()))?;

        let new_imgpath = dist.join(file_name);

        fs::rename(now_imgpath, &new_imgpath).with_context(|| {
            format!(
                "move failed from: {}; to: {};",
                now_imgpath.display(),
                new_imgpath.display()
            )
        })?;

        self.last_action_message = format!(
            "move: {}",
            dist.file_name()
                .context("failed get file neme")?
                .to_str()
                .context("failed convert file neme to str")?
        );

        Ok(())
    }

    fn next_image(&mut self) {
        if !self.is_finished() {
            self.idx += 1;
        }
    }

    fn is_finished(&self) -> bool {
        self.idx >= self.images.len()
    }

    fn set_imgstate(&mut self) -> Result<()> {
        let picker = Picker::from_fontsize((8, 12));
        let path = self.images.get(self.idx).unwrap();
        let dyn_img = ImageReader::open(path)?.decode()?;
        self.imgstate = Some(picker.new_resize_protocol(dyn_img));
        Ok(())
    }

    fn get_imgname(&self) -> Option<String> {
        self.images
            .get(self.idx)
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
    }
}

fn main() -> Result<()> {
    // config読み込み
    let config_str = fs::read_to_string("config.toml").context("config.toml not found")?;
    let config: Config = toml::from_str(&config_str).context("config.toml not valid toml")?;

    let mut app = App::new(config)?;

    // ターミナル設定
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // メインループ
    let res = run_app(&mut terminal, &mut app);

    // 終了処理
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("error occurred: {:?}", err);
        Err(err)
    } else {
        println!("exit successfully");
        Ok(())
    }
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        // UIの描画
        let _ = app.set_imgstate();
        terminal.draw(|f| ui(f, app))?;

        // イベントのポーリング
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char(c) => app.on_key(c)?,
                    _ => {}
                }
            }
        }

        if app.should_quit || app.is_finished() {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    // 画像情報とキーバインド表示に分割
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(f.area());

    // 画像とその情報
    let img_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // 画像(可変領域)
            Constraint::Length(3), // ファイル情報
        ])
        .split(main_chunks[0]);

    let image_block = Block::default().title("Image").borders(Borders::ALL);
    f.render_widget(image_block, img_chunks[0]);

    if app.is_finished() {
        let centered_rect = centered_rect(60, 20, img_chunks[0]);
        let all_done_text = Paragraph::new("all image done !!")
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("done"))
            .wrap(Wrap { trim: true })
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(all_done_text, centered_rect);
    } else {
        let centered_rect = centered_rect(60, 20, img_chunks[0]);
        let image = StatefulImage::default();
        let state = app.imgstate.as_mut().unwrap();
        f.render_stateful_widget(image, centered_rect, state);
    }

    let file_info_text = format!(
        "file: {}\nprogress: {} / {}",
        app.get_imgname().unwrap_or_else(|| "N/A".to_string()),
        app.idx + 1,
        app.images.len()
    );
    let file_info_widget =
        Paragraph::new(file_info_text).block(Block::default().title("Info").borders(Borders::ALL));
    f.render_widget(file_info_widget, img_chunks[1]);

    // --- 右側領域（キーバインドとログ） ---
    let info_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(main_chunks[1]);

    // キーバインドのリストを作成
    let mut key_items: Vec<ListItem> = app
        .config
        .dists
        .iter()
        .map(|(key, folder)| {
            let text = format!("[{}] -> {}", key, folder);
            let style = if folder.to_lowercase() == "skip" {
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
        .block(Block::default().title("keybind").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_widget(keys_widget, info_chunks[0]);

    // アクションログ
    let log_widget = Paragraph::new(app.last_action_message.as_str())
        .block(Block::default().title("Last Action").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(log_widget, info_chunks[1]);
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
