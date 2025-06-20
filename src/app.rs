use anyhow::{anyhow, Context, Result};
use image::ImageReader;

use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc,
    },
    thread::{self, available_parallelism, JoinHandle},
};

use ratatui_image::{picker::Picker, protocol::StatefulProtocol};

use crate::Config;

struct ProcessedImg {
    state: StatefulProtocol,
    idx: usize,
}

struct App {
    // viewmodelの作成に直接関係
    config: Config,
    imgs: Vec<PathBuf>,
    rx: Receiver<ProcessedImg>,
    progress: usize,
    req_quit: bool,

    // privateより
    // next_idx: Arc<AtomicUsize>,
    handles: Vec<JoinHandle<()>>,
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

const PROCESSED_IMG_BUFSIZE: usize = 7;

impl App {
    pub fn new(config: Config) -> Result<Self> {
        // imagesの取得
        if !config.dir.is_dir() {
            return Err(anyhow!("dir is not valid: {}", config.dir.display()));
        }
        let imgs = find_images_in_dir(&config.dir)?;
        if imgs.is_empty() {
            return Err(anyhow!("no images found in dir: {}", config.dir.display()));
        }
        let img_num = imgs.len();

        // スレッド作成の準備
        let worker_num = match available_parallelism() {
            Ok(n) => n.get() - 1,
            Err(_) => 1,
        };

        let (tx, rx) = sync_channel::<ProcessedImg>(PROCESSED_IMG_BUFSIZE);
        let picker = Picker::from_query_stdio().unwrap_or(Picker::from_fontsize((8, 14)));
        let next_idx = Arc::new(AtomicUsize::new(0));

        // スレッド作成
        let mut handles: Vec<JoinHandle<()>> = Vec::new();
        for _ in 0..worker_num {
            let thread_tx = tx.clone();
            let thread_next_idx = next_idx.clone();
            let thread_picker = picker.clone();
            let thread_imgs = &imgs;
            let handle = thread::spawn(move || loop {
                let idx = thread_next_idx.fetch_add(1, Ordering::Relaxed);
                if idx >= img_num {
                    break;
                }

                // 画像処理
                let Ok(reader) = ImageReader::open(&thread_imgs[idx]) else {
                    eprintln!("cannot open file {}", thread_imgs[idx].display());
                    continue;
                };

                let Ok(dynamic_img) = reader.decode() else {
                    eprintln!("cannot decpde image {}", thread_imgs[idx].display());
                    continue;
                };

                let state = thread_picker.new_resize_protocol(dynamic_img);

                if thread_tx.send(ProcessedImg { state, idx }).is_err() {
                    break;
                }
            });
            handles.push(handle);
        }
        drop(tx);

        let app = App {
            config,
            imgs,
            rx,
            progress: 0,
            req_quit: false,
            handles,
        };

        return Ok(app);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        while let Some(handle) = self.handles.pop() {
            handle.join().unwrap();
        }
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
