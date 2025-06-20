use anyhow::Result;

use std::{collections::HashMap, path::PathBuf};

use ratatui_image::protocol::StatefulProtocol;

use crate::app::{App, AppLog};
struct ViewModel {
    // 画像
    img: StatefulProtocol,
    // 画像情報
    img_path: PathBuf,
    progress: usize,
    img_num: usize,
    // キーバインド
    keybind: HashMap<char, PathBuf>,
    // ログ
    log: Option<AppLog>,
}

// modelからのfrom
impl ViewModel {
    fn from_app(app: &mut App) -> Result<Self> {
        let img_info = app.get_img()?;
        let app_info = app.get_app_info();
        Ok(ViewModel {
            img: img_info.state,
            img_path: img_info.path,
            progress: app.progress,
            img_num: app_info.img_num,
            keybind: app_info.keybind,
            log: app.log,
        })
    }
}
