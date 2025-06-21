use anyhow::{anyhow, Context, Result};
use image::ImageReader;

use std::{
    cmp::max,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{sync_channel, Receiver},
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

pub struct App {
    // viewmodelの作成に直接関係
    config: Config,
    imgs: Arc<Vec<PathBuf>>,
    rx: Receiver<ProcessedImg>,
    pub log: Option<AppLog>,

    idx: usize,
    handles: Vec<JoinHandle<()>>,
}

pub struct ImgInfo {
    pub state: StatefulProtocol,
    pub path: PathBuf,
}

pub struct AppInfo {
    pub img_num: usize,
    pub keybind: HashMap<char, PathBuf>,
}

#[derive(Clone)]
pub enum AppLog {
    MoveSuccess(PathBuf, PathBuf),
    Skip(PathBuf),
}

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

        // うまく使わない方法を模索している
        // 不変参照かつAppのほうが長生きな気がするので
        let imgs = Arc::new(imgs);

        // スレッド作成の準備
        let worker_num = match available_parallelism() {
            Ok(n) => max(n.get() - 1, 1),
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
            let thread_imgs = imgs.clone();
            let thread_picker = picker.clone();
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
            imgs: imgs,
            rx,
            log: None,
            idx: 0,
            handles,
        };

        return Ok(app);
    }

    pub fn get_img(&mut self) -> Result<ImgInfo> {
        match self.rx.recv() {
            Ok(r) => Ok({
                self.idx = r.idx;
                ImgInfo {
                    state: r.state,
                    path: self.imgs[r.idx].clone(),
                }
            }),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_app_info(&self) -> AppInfo {
        AppInfo {
            img_num: self.imgs.len(),
            keybind: self.config.dests.clone(),
        }
    }

    /// キー入力に基づいてアクションを実行する
    pub fn on_key(&mut self, key: char) -> Result<()> {
        if let Some(dest) = self.config.dests.get(&key) {
            // "skip" は特別扱い
            if dest == Path::new("skip") {
                self.log = Some(AppLog::Skip(
                    self.imgs[self.idx]
                        .file_name()
                        .context("skip filename cannot get")?
                        .into(),
                ));
            } else {
                let log = self.move_img(dest, &self.imgs[self.idx])?;
                self.log = Some(log);
            }
        }
        Ok(())
    }

    /// 現在の画像を新しいディレクトリに移動する
    fn move_img(&self, dest: &Path, src: &Path) -> Result<AppLog> {
        let file_name = src.file_name().context("Failed to get file name")?;

        fs::create_dir_all(dest).with_context(|| {
            format!("Failed to create destination directory: {}", dest.display())
        })?;

        let dest = dest.join(file_name);

        if !dest.exists() {
            fs::rename(src, &dest).with_context(|| {
                format!(
                    "Failed to move image from {} to {}",
                    src.display(),
                    dest.display()
                )
            })?;
        } else {
            return Err(anyhow!("move destination has same name file"));
        }

        Ok(AppLog::MoveSuccess(file_name.into(), dest))
    }
}

impl Drop for App {
    fn drop(&mut self) {
        while let Some(handle) = self.handles.pop() {
            if let Err(e) = handle.join() {
                eprintln!("error in thread {:?}", e);
            }
        }
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
