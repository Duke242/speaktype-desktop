//! Speech model catalog and downloads.
//!
//! - Whisper: GGML files that whisper.cpp publishes on Hugging Face. On macOS
//!   each also gets a CoreML encoder so it can run on the Neural Engine.
//! - Parakeet: int8 ONNX exports of NVIDIA's Parakeet TDT, one folder of files.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::platform;

const WHISPER_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Files in a Parakeet model folder, as `ParakeetModel::load` expects them.
const PARAKEET_FILES: &[&str] = &[
    "nemo128.onnx",
    "vocab.txt",
    "decoder_joint-model.int8.onnx",
    "encoder-model.int8.onnx",
];

/// Languages Parakeet TDT v3 transcribes.
const PARAKEET_V3_LANGUAGES: &[&str] = &[
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineKind {
    Whisper,
    Parakeet,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub engine: EngineKind,
    /// Whisper: the GGML file name. Parakeet: the folder name.
    #[serde(skip)]
    pub file: &'static str,
    /// Hugging Face repo for Parakeet files.
    #[serde(skip)]
    pub repo: &'static str,
    /// CoreML encoder for the Neural Engine, as published next to the GGML file.
    #[serde(skip)]
    pub coreml_encoder: Option<&'static str>,
    /// Download size of the model itself, without the Neural Engine companion.
    pub size_mb: u32,
    /// Extra download for Neural Engine acceleration, where available.
    pub accelerator_mb: u32,
    pub english_only: bool,
    /// Languages the model can transcribe, or `None` for every Whisper language.
    pub languages: Option<&'static [&'static str]>,
    pub description: &'static str,
    /// Relative scores out of 10, shown as bars in AI Models.
    pub speed: f64,
    pub accuracy: f64,
    pub min_ram_gb: u32,
}

/// Neural Engine files up to this size download with the model. Larger ones
/// (1.1 GB for Turbo) are an optional extra the user adds from AI Models.
const AUTO_ACCELERATOR_MB: u32 = 200;

impl ModelInfo {
    pub fn supports_language(&self, language: &str) -> bool {
        language == "auto" || self.languages.is_none_or(|languages| languages.contains(&language))
    }
}

const WHISPER: ModelInfo = ModelInfo {
    id: "",
    name: "",
    engine: EngineKind::Whisper,
    file: "",
    repo: "",
    coreml_encoder: None,
    size_mb: 0,
    accelerator_mb: 0,
    english_only: false,
    languages: None,
    description: "",
    speed: 0.0,
    accuracy: 0.0,
    min_ram_gb: 0,
};

pub const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "parakeet-tdt-v3",
        name: "Parakeet v3",
        engine: EngineKind::Parakeet,
        file: "parakeet-tdt-0.6b-v3-int8",
        repo: "istupakov/parakeet-tdt-0.6b-v3-onnx",
        size_mb: 640,
        languages: Some(PARAKEET_V3_LANGUAGES),
        description: "Fastest model, with punctuation. English and 24 European languages.",
        speed: 9.7,
        accuracy: 9.2,
        min_ram_gb: 4,
        ..WHISPER
    },
    ModelInfo {
        id: "parakeet-tdt-v2",
        name: "Parakeet v2 (English)",
        engine: EngineKind::Parakeet,
        file: "parakeet-tdt-0.6b-v2-int8",
        repo: "istupakov/parakeet-tdt-0.6b-v2-onnx",
        size_mb: 631,
        english_only: true,
        languages: Some(&["en"]),
        description: "Very fast and accurate for English.",
        speed: 9.8,
        accuracy: 9.1,
        min_ram_gb: 4,
        ..WHISPER
    },
    ModelInfo {
        id: "large-v3-turbo-q5",
        name: "Whisper Large v3 Turbo (compressed)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        coreml_encoder: Some("ggml-large-v3-turbo-encoder.mlmodelc.zip"),
        size_mb: 547,
        accelerator_mb: 1119,
        description: "Near-flagship accuracy at a third of the size. Great for dictation in any language.",
        speed: 7.5,
        accuracy: 9.4,
        min_ram_gb: 6,
        ..WHISPER
    },
    ModelInfo {
        id: "large-v3-turbo",
        name: "Whisper Large v3 Turbo",
        file: "ggml-large-v3-turbo.bin",
        coreml_encoder: Some("ggml-large-v3-turbo-encoder.mlmodelc.zip"),
        size_mb: 1624,
        accelerator_mb: 1119,
        description: "The most accurate Whisper model. Best on machines with a fast GPU.",
        speed: 7.0,
        accuracy: 9.5,
        min_ram_gb: 8,
        ..WHISPER
    },
    ModelInfo {
        id: "small-en",
        name: "Whisper Small (English)",
        file: "ggml-small.en.bin",
        coreml_encoder: Some("ggml-small.en-encoder.mlmodelc.zip"),
        size_mb: 466,
        accelerator_mb: 155,
        english_only: true,
        languages: Some(&["en"]),
        description: "Good balance of speed and accuracy for English.",
        speed: 8.0,
        accuracy: 8.5,
        min_ram_gb: 4,
        ..WHISPER
    },
    ModelInfo {
        id: "base-en",
        name: "Whisper Base (English)",
        file: "ggml-base.en.bin",
        coreml_encoder: Some("ggml-base.en-encoder.mlmodelc.zip"),
        size_mb: 142,
        accelerator_mb: 36,
        english_only: true,
        languages: Some(&["en"]),
        description: "Fast on any machine. Fine for short English dictation.",
        speed: 9.0,
        accuracy: 7.5,
        min_ram_gb: 2,
        ..WHISPER
    },
    ModelInfo {
        id: "base",
        name: "Whisper Base",
        file: "ggml-base.bin",
        coreml_encoder: Some("ggml-base-encoder.mlmodelc.zip"),
        size_mb: 142,
        accelerator_mb: 36,
        description: "Small and quick to download. A good way to get started in any language.",
        speed: 9.0,
        accuracy: 7.3,
        min_ram_gb: 2,
        ..WHISPER
    },
    ModelInfo {
        id: "tiny",
        name: "Whisper Tiny",
        file: "ggml-tiny.bin",
        coreml_encoder: Some("ggml-tiny-encoder.mlmodelc.zip"),
        size_mb: 75,
        accelerator_mb: 14,
        description: "Fastest Whisper model and least accurate. Useful for testing.",
        speed: 9.5,
        accuracy: 6.0,
        min_ram_gb: 2,
        ..WHISPER
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
    /// Ready to transcribe.
    pub downloaded: bool,
    pub downloading: bool,
    /// Whether Neural Engine acceleration applies on this computer, and is installed.
    pub accelerator: Accelerator,
    /// What a fresh download fetches on this computer, including any Neural Engine files.
    pub download_mb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Accelerator {
    /// Not offered for this model on this OS.
    None,
    Missing,
    Installed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
}

/// One file to fetch for a model.
struct Asset {
    url: String,
    /// Where the finished file (or, for zips, the extracted folder) ends up.
    dest: PathBuf,
    /// Zipped folders are extracted next to `dest` and the zip is removed.
    zipped: bool,
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

    /// The Whisper file or Parakeet folder the engine loads.
    pub fn path(&self, model: &ModelInfo) -> PathBuf {
        self.dir.join(model.file)
    }

    fn coreml_path(&self, model: &ModelInfo) -> Option<PathBuf> {
        let zip = model.coreml_encoder.filter(|_| platform::NEURAL_ENGINE)?;
        Some(self.dir.join(zip.trim_end_matches(".zip")))
    }

    /// Files for a model on this OS. The Neural Engine companion is included when
    /// `with_accelerator` is true.
    fn assets(&self, model: &ModelInfo, with_accelerator: bool) -> Vec<Asset> {
        match model.engine {
            EngineKind::Whisper => {
                let mut assets = vec![Asset {
                    url: format!("{WHISPER_BASE_URL}/{}", model.file),
                    dest: self.path(model),
                    zipped: false,
                }];
                if let (true, Some(zip), Some(dest)) =
                    (with_accelerator, model.coreml_encoder, self.coreml_path(model))
                {
                    assets.push(Asset {
                        url: format!("{WHISPER_BASE_URL}/{zip}"),
                        dest,
                        zipped: true,
                    });
                }
                assets
            }
            EngineKind::Parakeet => PARAKEET_FILES
                .iter()
                .map(|file| Asset {
                    url: format!("https://huggingface.co/{}/resolve/main/{file}", model.repo),
                    dest: self.path(model).join(file),
                    zipped: false,
                })
                .collect(),
        }
    }

    /// Ready to transcribe. Files only appear under their final names once
    /// complete, so presence means the download finished.
    pub fn is_downloaded(&self, id: &str) -> bool {
        find(id).is_some_and(|m| self.is_ready(m))
    }

    fn is_ready(&self, model: &ModelInfo) -> bool {
        match model.engine {
            EngineKind::Whisper => self.path(model).is_file(),
            EngineKind::Parakeet => PARAKEET_FILES.iter().all(|f| self.path(model).join(f).is_file()),
        }
    }

    fn accelerator(&self, model: &ModelInfo) -> Accelerator {
        match self.coreml_path(model) {
            None => Accelerator::None,
            Some(path) if path.is_dir() => Accelerator::Installed,
            Some(_) => Accelerator::Missing,
        }
    }

    pub fn statuses(&self) -> Vec<ModelStatus> {
        let active = self.active.lock().unwrap();
        CATALOG
            .iter()
            .map(|m| ModelStatus {
                info: *m,
                downloaded: self.is_ready(m),
                downloading: active.contains_key(m.id),
                accelerator: self.accelerator(m),
                download_mb: m.size_mb
                    + if auto_accelerator(m) { m.accelerator_mb } else { 0 },
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
        let mut paths = vec![self.path(model)];
        // Large Turbo and its compressed build share one encoder; keep it while either is installed.
        if let Some(coreml) = self.coreml_path(model) {
            let shared = CATALOG.iter().any(|other| {
                other.id != model.id && other.coreml_encoder == model.coreml_encoder && self.is_ready(other)
            });
            if !shared {
                paths.push(coreml);
            }
        }
        for path in paths {
            let result = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(e) = result
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(e.to_string());
            }
        }
        Ok(())
    }

    /// Downloads whatever the model is still missing, reporting combined progress
    /// a few times a second. `accelerator` asks for the Neural Engine files; by
    /// default they're included only when small. Also adds them to an
    /// already-installed Whisper model.
    pub async fn download(
        &self,
        id: &str,
        accelerator: Option<bool>,
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

        let with_accelerator = accelerator.unwrap_or_else(|| auto_accelerator(model));
        let result = self
            .fetch_missing(model, with_accelerator, &cancelled, &on_progress)
            .await;
        self.active.lock().unwrap().remove(id);
        result
    }

    async fn fetch_missing(
        &self,
        model: &ModelInfo,
        with_accelerator: bool,
        cancelled: &AtomicBool,
        on_progress: &impl Fn(DownloadProgress),
    ) -> Result<(), String> {
        let missing: Vec<Asset> = self
            .assets(model, with_accelerator)
            .into_iter()
            .filter(|asset| !asset.dest.exists())
            .collect();
        if missing.is_empty() {
            return Ok(());
        }

        let client = reqwest::Client::new();
        let mut sizes = Vec::with_capacity(missing.len());
        for asset in &missing {
            let size = client
                .head(&asset.url)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|e| format!("Download failed: {e}"))?
                .content_length()
                .unwrap_or(0);
            sizes.push(size);
        }
        let total: u64 = sizes.iter().sum();

        let mut done = 0;
        let report = |downloaded: u64| {
            on_progress(DownloadProgress {
                id: model.id.into(),
                downloaded,
                total,
            })
        };
        for (asset, size) in missing.iter().zip(sizes) {
            fetch_asset(&client, asset, size, cancelled, &|bytes| report(done + bytes)).await?;
            done += size;
        }
        report(total);
        Ok(())
    }
}

async fn fetch_asset(
    client: &reqwest::Client,
    asset: &Asset,
    expected: u64,
    cancelled: &AtomicBool,
    on_bytes: &impl Fn(u64),
) -> Result<(), String> {
    let dir = asset.dest.parent().ok_or("Invalid model path")?;
    tokio::fs::create_dir_all(dir).await.map_err(|e| e.to_string())?;
    let part = with_suffix(&asset.dest, if asset.zipped { ".zip.part" } else { ".part" });

    let response = client
        .get(&asset.url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Download failed: {e}"))?;
    let mut file = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
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
                on_bytes(downloaded);
                last_report = Instant::now();
            }
        }
        file.flush().await.map_err(|e| e.to_string())?;
        if expected > 0 && downloaded != expected {
            return Err(format!("Download incomplete: got {downloaded} of {expected} bytes"));
        }
        Ok(())
    }
    .await;
    drop(file);

    if let Err(e) = outcome {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }

    if asset.zipped {
        // Extract into a temporary folder, then move the result into place, so a
        // half-extracted folder never looks installed.
        let staging = with_suffix(&asset.dest, ".extracting");
        let _ = tokio::fs::remove_dir_all(&staging).await;
        let result = platform::extract_zip(&part, &staging).and_then(|()| {
            let name = asset.dest.file_name().ok_or("Invalid model path")?;
            std::fs::rename(staging.join(name), &asset.dest).map_err(|e| e.to_string())
        });
        let _ = tokio::fs::remove_file(&part).await;
        let _ = tokio::fs::remove_dir_all(&staging).await;
        result.map_err(|e| format!("Couldn't unpack the Neural Engine files: {e}"))
    } else {
        tokio::fs::rename(&part, &asset.dest).await.map_err(|e| e.to_string())
    }
}

/// Whether a model's Neural Engine files download with it by default on this OS.
fn auto_accelerator(model: &ModelInfo) -> bool {
    platform::NEURAL_ENGINE && model.coreml_encoder.is_some() && model.accelerator_mb <= AUTO_ACCELERATOR_MB
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique() {
        let mut ids: Vec<_> = CATALOG.iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
    }

    #[test]
    fn language_support() {
        let v3 = find("parakeet-tdt-v3").unwrap();
        assert!(v3.supports_language("auto"));
        assert!(v3.supports_language("de"));
        assert!(!v3.supports_language("hi"));
        assert!(find("large-v3-turbo").unwrap().supports_language("hi"));
        assert!(!find("small-en").unwrap().supports_language("fr"));
    }

    #[test]
    fn parakeet_assets_live_in_one_folder() {
        let store = ModelStore::new(PathBuf::from("/models"));
        let assets = store.assets(find("parakeet-tdt-v2").unwrap(), true);
        assert_eq!(assets.len(), PARAKEET_FILES.len());
        assert!(assets.iter().all(|a| a.dest.starts_with("/models/parakeet-tdt-0.6b-v2-int8")));
        assert!(assets[0].url.starts_with("https://huggingface.co/istupakov/parakeet-tdt-0.6b-v2-onnx/"));
    }
}
