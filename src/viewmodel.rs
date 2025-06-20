use anyhow::Result;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use ratatui_image::protocol::StatefulProtocol;

use crate::App2;

struct ViewModel {
    // 画像
    img: StatefulProtocol,
    // 画像情報
    img_path: PathBuf,
    progress: usize,
    img_num: usize,
    // キーバインド
    keybind: HashMap<char, String>,
    // ログ
    log: Result<()>,
}

// modelからのfrom
impl ViewModel {
    fn from_app(app: &App2) -> Self {}
}
