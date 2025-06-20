use anyhow::Result;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use ratatui_image::protocol::StatefulProtocol;

struct App {
    rx: Receiver<ProcessedImg>,
    progress: usize,
    img_num: usize,
    config: Config,
    req_quit: bool,
}
