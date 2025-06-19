use anyhow::{anyhow, Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::ImageReader;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(help = "path to config.toml", value_name = "FILE")]
    config: Option<PathBuf>,
}

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
    image_changed: bool,
    picker: Picker,
}

impl App {
    fn new(config: Config) -> Result<Self> {
        let dir = Path::new(&config.dir);
        if !dir.exists() || !dir.is_dir() {
            return Err(anyhow::anyhow!("dir is not valid: {}", config.dir));
        }

        let images = find_images_in_dir(dir)?;

        if images.is_empty() {
            return Err(anyhow::anyhow!("no images found in dir: {}", config.dir));
        }

        Ok(Self {
            images,
            imgstate: None,
            idx: 0,
            config,
            should_quit: false,
            last_action_message: String::new(),
            image_changed: true, // 最初の画像を読み込むためにtrueに設定
            picker: Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((8, 14))),
        })
    }

    /// キー入力に基づいてアクションを実行する
    fn on_key(&mut self, key: char) -> Result<()> {
        if let Some(dist) = self.config.dists.get(&key) {
            // "skip" は特別扱い
            if dist == "skip" {
                self.last_action_message =
                    format!("skip: {}", self.get_imgname().unwrap_or_default());
                self.next_image();
            } else {
                self.move_current_image(&dist.clone())?;
                self.next_image();
            }
        }
        Ok(())
    }

    /// 現在の画像を新しいディレクトリに移動する
    fn move_current_image(&mut self, dist: &str) -> Result<()> {
        if self.is_finished() {
            return Ok(());
        }

        let current_image_path = &self.images[self.idx];
        let file_name = current_image_path
            .file_name()
            .context("Failed to get file name")?;

        let dist = Path::new(dist);
        fs::create_dir_all(dist)
            .with_context(|| format!("Failed to create dist directory: {}", dist.display()))?;

        let new_image_path = dist.join(file_name);

        if !new_image_path.exists() {
            fs::rename(current_image_path, &new_image_path).with_context(|| {
                format!(
                    "Failed to move image from {} to {}",
                    current_image_path.display(),
                    new_image_path.display()
                )
            })?;
        } else {
            return Err(anyhow!("move distination has same name file"));
        }

        self.last_action_message = format!(
            "move: {} -> {}",
            file_name.to_string_lossy(),
            dist.display()
        );

        Ok(())
    }

    /// 次の画像へインデックスを進める
    fn next_image(&mut self) {
        if !self.is_finished() {
            self.idx += 1;
            self.image_changed = true; // 画像が変更されたことをマーク
        }
    }

    /// すべての画像の処理が完了したか
    fn is_finished(&self) -> bool {
        self.idx >= self.images.len()
    }

    /// 表示する画像の状態を更新する
    fn update_imgstate(&mut self) -> Result<()> {
        if self.image_changed && !self.is_finished() {
            let picker = &self.picker;
            if let Some(path) = self.images.get(self.idx) {
                let dyn_img = ImageReader::open(path)?.decode()?;
                self.imgstate = Some(picker.new_resize_protocol(dyn_img));
            }
            self.image_changed = false; // フラグをリセット
        }
        Ok(())
    }

    /// 現在の画像ファイル名を取得する
    fn get_imgname(&self) -> Option<String> {
        self.images
            .get(self.idx)
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
    }
}

/// 指定されたディレクトリから画像ファイルの一覧を取得する
fn find_images_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let img_extensions = ["jpg", "jpeg", "png", "gif", "bmp"];
    let images = fs::read_dir(dir)
        .with_context(|| format!("cannot read dir: {}", dir.display()))?
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
    Ok(images)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 設定ファイルの読み込み
    let config_str = fs::read_to_string(cli.config.unwrap_or("config.toml".into()))
        .context("config.toml not found or unreadable")?;
    let config: Config = toml::from_str(&config_str).context("config.toml is not valid toml")?;

    let mut app = App::new(config)?;

    // ターミナル設定
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
        eprintln!("error occurred: {:?}", err);
        Err(err)
    } else {
        println!("exit successfully");
        Ok(())
    }
}

/// メインループ
fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    let mut pressed_keys: HashSet<KeyCode> = HashSet::new();
    loop {
        // 必要に応じて画像データを更新
        app.update_imgstate()?;

        // UIの描画
        terminal.draw(|f| ui(f, app))?;

        // イベントのポーリング
        if event::poll(Duration::from_millis(10))? {
            // キーが押された瞬間のみを捉える

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char(c) => {
                            if !pressed_keys.contains(&key.code) {
                                app.on_key(c)?;
                                pressed_keys.insert(key.code);
                            }
                        }
                        _ => {}
                    }
                } else if key.kind == KeyEventKind::Release {
                    match key.code {
                        KeyCode::Char(_) => {
                            if pressed_keys.contains(&key.code) {
                                pressed_keys.remove(&key.code);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit || app.is_finished() {
            // 終了前に最後の状態を描画するため少し待つ
            if app.is_finished() {
                terminal.draw(|f| ui(f, app))?;
                std::thread::sleep(Duration::from_secs(1));
            }
            return Ok(());
        }
    }
}

/// UIを描画する
fn ui(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    draw_image_panel(f, app, main_chunks[0]);
    draw_info_panel(f, app, main_chunks[1]);
}

/// 画像表示エリアを描画する
fn draw_image_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let image_block = Block::default().title("Image").borders(Borders::ALL);
    f.render_widget(image_block, chunks[0]);

    if app.is_finished() {
        let done_block = Block::default().borders(Borders::ALL).title("Done");
        let text = Paragraph::new("All images have been sorted!")
            .style(Style::default().fg(Color::Green))
            .block(done_block)
            .alignment(Alignment::Center);
        f.render_widget(text, centered_rect(60, 20, chunks[0]));
    } else if let Some(state) = app.imgstate.as_mut() {
        let image = StatefulImage::default();
        f.render_stateful_widget(image, chunks[0], state);
    }

    let file_info_text = format!(
        "File: {}\nProgress: {} / {}",
        app.get_imgname().unwrap_or_else(|| "N/A".to_string()),
        if app.is_finished() {
            app.idx
        } else {
            app.idx + 1
        },
        app.images.len()
    );
    let file_info_widget =
        Paragraph::new(file_info_text).block(Block::default().title("Info").borders(Borders::ALL));
    f.render_widget(file_info_widget, chunks[1]);
}

/// 情報エリアを描画する
fn draw_info_panel(f: &mut Frame, app: &App, area: Rect) {
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
