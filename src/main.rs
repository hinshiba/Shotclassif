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

use crate::app::App;

pub mod app;
pub mod ui;
pub mod viewmodel;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(help = "path to config.toml", value_name = "FILE")]
    config: Option<PathBuf>,
}

/// TOML file structure
#[derive(Deserialize, Debug)]
struct Config {
    dir: PathBuf,
    dists: HashMap<char, PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 設定ファイルの読み込み
    let config_str = fs::read_to_string(cli.config.unwrap_or("config.toml".into()))
        .context("config.toml not found or unreadable")?;
    let config: Config = toml::from_str(&config_str).context("config.toml is not valid toml")?;

    // ターミナル設定
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(config)?;

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
    app: &mut App2,
    images: &Vec<PathBuf>,
) -> Result<()> {
    let mut pressed_keys: HashSet<KeyCode> = HashSet::new();
    let image_num = images.len();
    // 必要に応じて画像データを更新
    app.update_imgstate()?;
    loop {
        // UIの描画
        terminal.draw(|f| viewmodel(f, app, false))?;

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
                terminal.draw(|f| viewmodel(f, app, true))?;
                std::thread::sleep(Duration::from_secs(1));
            }
            return Ok(());
        }
    }
}
