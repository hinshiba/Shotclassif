use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self},
    path::PathBuf,
    time::Duration,
};

use crate::viewmodel::ViewModel;
use crate::{app::App, ui::ui};

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
pub struct Config {
    dir: PathBuf,
    dests: HashMap<char, PathBuf>,
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

    let app = &mut App::new(config)?;
    let viewmodel = &mut ViewModel::new_from_app(app)?;
    let mut pressed_keys: HashSet<KeyCode> = HashSet::new();
    // メインループ
    loop {
        // 描画
        terminal.draw(|f| ui(f, viewmodel))?;

        // イベントのポーリング
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char(c) => {
                            if !pressed_keys.contains(&key.code) {
                                let Ok(_) = viewmodel.on_key(app, c) else {
                                    continue;
                                };
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
    }

    // 終了処理
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
