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
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{mpsc::Receiver, Arc},
    time::Duration,
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread::available_parallelism,
};
use std::{sync::mpsc::sync_channel, thread};

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

struct ProcessedImg {
    state: StatefulProtocol,
    idx: usize,
}

struct App<'a> {
    recver: Receiver<ProcessedImg>,
    images: &'a Vec<PathBuf>,
    processed_num: usize,
    current_img: Option<ProcessedImg>,
    config: Config,
    should_quit: bool,
    last_action_message: String,
}

impl<'a> App<'a> {
    fn new(config: Config, recver: Receiver<ProcessedImg>, images: &'a Vec<PathBuf>) -> Self {
        Self {
            recver,
            images,
            processed_num: 0,
            current_img: None,
            config,
            should_quit: false,
            last_action_message: String::new(),
        }
    }

    /// キー入力に基づいてアクションを実行する
    fn on_key(&mut self, key: char) -> Result<()> {
        if let Some(dist) = self.config.dists.get(&key) {
            // "skip" は特別扱い
            if dist == "skip" {
                self.last_action_message = format!("skip: {}", self.get_imgname().display());
            } else {
                self.move_current_image(&dist.clone())?;
            }
        }
        let _ = self.update_imgstate();
        self.processed_num += 1;
        Ok(())
    }

    /// 現在の画像を新しいディレクトリに移動する
    fn move_current_image(&mut self, dist: &str) -> Result<()> {
        let current_image_path = self.get_imgname();
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

    /// 新しい画像にする
    fn update_imgstate(&mut self) -> Result<()> {
        let res = self.recver.recv()?;
        self.current_img = Some(res);
        Ok(())
    }

    fn get_imgname(&self) -> &PathBuf {
        &self.images[self.current_img.as_ref().unwrap().idx]
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

    // 画像のパス一覧の取得
    let dir = Path::new(&config.dir);
    if !dir.exists() || !dir.is_dir() {
        return Err(anyhow::anyhow!("dir is not valid: {}", config.dir));
    }
    let images = &find_images_in_dir(dir)?;
    if images.is_empty() {
        return Err(anyhow::anyhow!("no images found in dir: {}", config.dir));
    }
    let img_num = images.len();

    // ワーカーの設定
    let next_idx = Arc::new(AtomicUsize::new(0));
    let worker_num = available_parallelism()
        .unwrap_or(NonZeroUsize::new(1).unwrap())
        .get();

    let (tx, rx) = sync_channel::<ProcessedImg>(4);

    // ターミナル設定
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let picker = Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((8, 14)));

    let res = thread::scope(|s| {
        for _ in 0..worker_num {
            let tx = tx.clone();
            let next_idx = next_idx.clone();
            let picker = picker.clone();

            s.spawn(move || loop {
                let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                if idx >= img_num {
                    break;
                }

                // Stateを作成
                let Ok(reader) = ImageReader::open(&images[idx]) else {
                    eprintln!("cannot open file {}", images[idx].display());
                    continue;
                };

                let Ok(dynamic_img) = reader.decode() else {
                    eprintln!("cannot decpde image {}", images[idx].display());
                    continue;
                };

                let state = picker.new_resize_protocol(dynamic_img);

                if tx.send(ProcessedImg { state, idx }).is_err() {
                    break;
                }
            });
        }
        drop(tx);
        let mut app = App::new(config, rx, images);
        run_app(&mut terminal, &mut app, images)
    });

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
fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    images: &Vec<PathBuf>,
) -> Result<()> {
    let mut pressed_keys: HashSet<KeyCode> = HashSet::new();
    let image_num = images.len();
    // 必要に応じて画像データを更新
    app.update_imgstate()?;
    loop {
        // UIの描画
        terminal.draw(|f| ui(f, app, false))?;

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

        if app.should_quit || app.processed_num >= image_num {
            // 終了前に最後の状態を描画するため少し待つ
            if app.processed_num >= image_num {
                terminal.draw(|f| ui(f, app, true))?;
                std::thread::sleep(Duration::from_secs(1));
            }
            return Ok(());
        }
    }
}

/// UIを描画する
fn ui(f: &mut Frame, app: &mut App, isfin: bool) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(f.area());

    draw_image_panel(f, app, main_chunks[0], isfin);
    draw_info_panel(f, app, main_chunks[1]);
}

/// 画像表示エリアを描画する
fn draw_image_panel(f: &mut Frame, app: &mut App, area: Rect, isfin: bool) {
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
