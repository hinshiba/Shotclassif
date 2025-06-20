use anyhow::Result;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

use ratatui_image::protocol::StatefulProtocol;

use crate::Config;

struct ProcessedImg {
    state: StatefulProtocol,
    idx: usize,
}

struct App {
    config: Config,
    imgs: Vec<PathBuf>,
    rx: Receiver<ProcessedImg>,
    progress: usize,
    req_quit: bool,
}

// struct App2<'a> {
//     recver: Receiver<ProcessedImg>,
//     images: &'a Vec<PathBuf>,
//     processed_num: usize,
//     current_img: Option<ProcessedImg>,
//     config: Config,
//     should_quit: bool,
//     last_action_message: String,
// }

impl App {
    fn new(config: Config) -> Result<Self> {
        // imagesの取得
        let dir = Path::new(&config.dir);
        if !dir.is_dir() {
            return Err(anyhow::anyhow!("dir is not valid: {}", config.dir));
        }
        let images = &find_images_in_dir(dir)?;
        if images.is_empty() {
            return Err(anyhow::anyhow!("no images found in dir: {}", config.dir));
        }
        let img_num = images.len();
    }
}

// impl<'a> App2<'a> {
//     fn new(config: Config, recver: Receiver<ProcessedImg>, images: &'a Vec<PathBuf>) -> Self {
//         Self {
//             recver,
//             images,
//             processed_num: 0,
//             current_img: None,
//             config,
//             should_quit: false,
//             last_action_message: String::new(),
//         }
//     }

//     /// キー入力に基づいてアクションを実行する
//     fn on_key(&mut self, key: char) -> Result<()> {
//         if let Some(dist) = self.config.dists.get(&key) {
//             // "skip" は特別扱い
//             if dist == "skip" {
//                 self.last_action_message = format!("skip: {}", self.get_imgname().display());
//             } else {
//                 self.move_current_image(&dist.clone())?;
//             }
//         }
//         let _ = self.update_imgstate();
//         self.processed_num += 1;
//         Ok(())
//     }

//     /// 現在の画像を新しいディレクトリに移動する
//     fn move_current_image(&mut self, dist: &str) -> Result<()> {
//         let current_image_path = self.get_imgname();
//         let file_name = current_image_path
//             .file_name()
//             .context("Failed to get file name")?;

//         let dist = Path::new(dist);
//         fs::create_dir_all(dist)
//             .with_context(|| format!("Failed to create dist directory: {}", dist.display()))?;

//         let new_image_path = dist.join(file_name);

//         if !new_image_path.exists() {
//             fs::rename(current_image_path, &new_image_path).with_context(|| {
//                 format!(
//                     "Failed to move image from {} to {}",
//                     current_image_path.display(),
//                     new_image_path.display()
//                 )
//             })?;
//         } else {
//             return Err(anyhow!("move distination has same name file"));
//         }

//         self.last_action_message = format!(
//             "move: {} -> {}",
//             file_name.to_string_lossy(),
//             dist.display()
//         );

//         Ok(())
//     }

//     /// 新しい画像にする
//     fn update_imgstate(&mut self) -> Result<()> {
//         let res = self.recver.recv()?;
//         self.current_img = Some(res);
//         Ok(())
//     }

//     fn get_imgname(&self) -> &PathBuf {
//         &self.images[self.current_img.as_ref().unwrap().idx]
//     }
// }

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
