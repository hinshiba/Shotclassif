use anyhow::Result;

use std::{collections::HashMap, path::PathBuf};

use ratatui_image::protocol::StatefulProtocol;

use crate::app::{App, AppLog};
pub struct ViewModel {
    // 画像
    pub img: StatefulProtocol,
    // 画像情報
    pub img_path: PathBuf,
    pub progress: usize,
    pub img_num: usize,
    // キーバインド
    pub keybind: HashMap<char, PathBuf>,
    // ログ
    pub log: Option<AppLog>,
    // 終了画面か
    pub is_fin: bool,
}

// modelからのfrom
impl ViewModel {
    pub fn new_from_app(app: &mut App) -> Result<Self> {
        let img_info = app.get_img()?;
        let app_info = app.get_app_info();
        Ok(ViewModel {
            img: img_info.state,
            img_path: img_info.path,
            progress: 0,
            img_num: app_info.img_num,
            keybind: app_info.keybind,
            log: None,
            is_fin: false,
        })
    }

    pub fn on_key(&mut self, app: &mut App, key: char) -> Result<()> {
        app.on_key(key)?;
        let img_info = app.get_img()?;
        self.img = img_info.state;
        self.img_path = img_info.path;
        self.progress += 1;
        self.log = app.log.clone();
        self.is_fin = self.progress >= self.img_num;
        Ok(())
    }
}
