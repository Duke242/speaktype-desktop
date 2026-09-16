//! Whisper model catalog and downloads.
//!
//! The macOS app uses CoreML builds of these models. Here we use the GGML files
//! that whisper.cpp publishes on Hugging Face.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

const DOWNLOAD_BASE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    pub english_only: bool,
    pub description: &'static str,
    /// Relative scores out of 10, shown as bars in AI Models.
    pub speed: f64,
    pub accuracy: f64,
    pub min_ram_gb: u32,
}

pub const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "large-v3-turbo-q5",
        name: "Whisper Large v3 Turbo (compressed)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 547,
        english_only: false,
        description: "Near-flagship accuracy at a third of the size. Great for dictation in any language.",
        speed: 7.5,
        accuracy: 9.4,
        min_ram_gb: 6,
    },
    ModelInfo {
        id: "large-v3-turbo",
        name: "Whisper Large v3 Turbo",
        file: "ggml-large-v3-turbo.bin",
        size_mb: 1624,
        english_only: false,
        description: "The most accurate Whisper model. Best on machines with a fast GPU.",
        speed: 7.0,
        accuracy: 9.5,
        min_ram_gb: 8,
    },
    ModelInfo {
        id: "small-en",
        name: "Whisper Small (English)",
        file: "ggml-small.en.bin",
        size_mb: 466,
        english_only: true,
        description: "Good balance of speed and accuracy for English.",
        speed: 8.0,
        accuracy: 8.5,
        min_ram_gb: 4,
    },
    ModelInfo {
        id: "base-en",
        name: "Whisper Base (English)",
        file: "ggml-base.en.bin",
        size_mb: 142,
        english_only: true,
        description: "Fast on any machine. Fine for short English dictation.",
        speed: 9.0,
        accuracy: 7.5,
        min_ram_gb: 2,
    },
    ModelInfo {
        id: "base",
        name: "Whisper Base",
        file: "ggml-base.bin",
        size_mb: 142,
        english_only: false,
        description: "Small and quick to download. A good way to get started in any language.",
        speed: 9.0,
        accuracy: 7.3,
        min_ram_gb: 2,
    },
    ModelInfo {
        id: "tiny",
        name: "Whisper Tiny",
        file: "ggml-tiny.bin",
        size_mb: 75,
        english_only: false,
        description: "Fastest and least accurate. Useful for testing.",
        speed: 9.5,
        accuracy: 6.0,
        min_ram_gb: 2,
    },
];

pub fn find(id: &str) -> Option<&'static ModelInfo> {
    CATALOG.iter().find(|m| m.id == id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    pub info: ModelInfo,
    pub downloaded: bool,
    pub downloading: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
}

pub struct ModelStore {
    dir: PathBuf,
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ModelStore {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            active: Mutex::new(HashMap::new()),
        }
    }

    pub fn path(&self, model: &ModelInfo) -> PathBuf {
        self.dir.join(model.file)
    }

    /// A model counts as downloaded once its file has been moved into place,
    /// which only happens after the whole download succeeded.
    pub fn is_downloaded(&self, id: &str) -> bool {
        find(id).is_some_and(|m| self.path(m).is_file())
    }

    pub fn statuses(&self) -> Vec<ModelStatus> {
        let active = self.active.lock().unwrap();
        CATALOG
            .iter()
            .map(|m| ModelStatus {
                info: *m,
                downloaded: self.path(m).is_file(),
                downloading: active.contains_key(m.id),
            })
            .collect()
    }

    pub fn cancel(&self, id: &str) {
        if let Some(flag) = self.active.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let model = find(id).ok_or("Unknown model")?;
        match std::fs::remove_file(self.path(model)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    /// Downloads a model to a `.part` file and renames it when complete.
    /// `on_progress` is called a few times a second.
    pub async fn download(
        &self,
        id: &str,
        on_progress: impl Fn(DownloadProgress),
    ) -> Result<(), String> {
        let model = find(id).ok_or("Unknown model")?;
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active.lock().unwrap();
            if active.contains_key(id) {
                return Err("This model is already downloading".into());
            }
            active.insert(id.to_string(), cancelled.clone());
        }

        let result = self.fetch(model, &cancelled, &on_progress).await;
        self.active.lock().unwrap().remove(id);
        result
    }

    async fn fetch(
        &self,
        model: &ModelInfo,
        cancelled: &AtomicBool,
        on_progress: &impl Fn(DownloadProgress),
    ) -> Result<(), String> {
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| e.to_string())?;
        let final_path = self.path(model);
        let part_path = final_path.with_extension("bin.part");

        let response = reqwest::get(format!("{DOWNLOAD_BASE}/{}", model.file))
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("Download failed: {e}"))?;
        let total = response
            .content_length()
            .unwrap_or(model.size_mb as u64 * 1024 * 1024);

        let mut file = tokio::fs::File::create(&part_path)
            .await
            .map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();
        let mut downloaded = 0u64;
        let mut last_report = Instant::now();

        let outcome: Result<(), String> = async {
            while let Some(chunk) = stream.next().await {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("Download cancelled".into());
                }
                let chunk = chunk.map_err(|e| format!("Download interrupted: {e}"))?;
                file.write_all(&chunk).await.map_err(|e| e.to_string())?;
                downloaded += chunk.len() as u64;
                if last_report.elapsed() >= Duration::from_millis(200) {
                    on_progress(DownloadProgress {
                        id: model.id.into(),
                        downloaded,
                        total,
                    });
                    last_report = Instant::now();
                }
            }
            file.flush().await.map_err(|e| e.to_string())?;
            if downloaded != total {
                return Err(format!(
                    "Download incomplete: got {downloaded} of {total} bytes"
                ));
            }
            Ok(())
        }
        .await;

        drop(file);
        match outcome {
            Ok(()) => {
                on_progress(DownloadProgress {
                    id: model.id.into(),
                    downloaded,
                    total,
                });
                tokio::fs::rename(&part_path, &final_path)
                    .await
                    .map_err(|e| e.to_string())
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&part_path).await;
                Err(e)
            }
        }
    }
}
