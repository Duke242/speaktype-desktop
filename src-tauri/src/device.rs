//! What this computer can run, and which model to recommend for it.

use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::models::{CATALOG, ModelInfo};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub chip: String,
    pub ram_gb: u32,
    pub cores: usize,
    /// Whether whisper.cpp runs on a GPU in this build.
    pub gpu: bool,
    /// A short line like "Apple M3 Pro · 18 GB · Metal".
    pub summary: String,
    /// 0..1 estimate of how fast this machine runs models.
    pub performance_tier: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub model_id: &'static str,
    pub reason: String,
}

pub fn detect() -> DeviceInfo {
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    let chip = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "this computer".into());
    let ram_gb = (system.total_memory() as f64 / 1024f64.powi(3)).round() as u32;
    let cores = System::physical_core_count().unwrap_or(4);
    let backend = gpu_backend();
    let mut summary = format!("{chip} · {ram_gb} GB");
    if let Some(backend) = backend {
        summary.push_str(&format!(" · {backend}"));
    }
    DeviceInfo {
        performance_tier: performance_tier(&chip, cores, backend.is_some()),
        chip,
        ram_gb,
        cores,
        gpu: backend.is_some(),
        summary,
    }
}

fn gpu_backend() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("Metal")
    } else if cfg!(feature = "cuda") {
        Some("CUDA")
    } else if cfg!(feature = "vulkan") {
        Some("Vulkan")
    } else {
        None
    }
}

/// Same shape as the macOS app's estimate: newer Apple Silicon scores higher,
/// and more cores add a little.
fn performance_tier(chip: &str, cores: usize, gpu: bool) -> f64 {
    let base = match apple_silicon_generation(chip) {
        Some(generation) => (0.6 + (generation as f64 - 1.0) * 0.12).min(1.0),
        None if gpu => 0.6,
        None => 0.5,
    };
    let core_bonus = ((cores as f64 - 4.0) * 0.02).clamp(0.0, 0.15);
    (base + core_bonus).min(1.0)
}

fn apple_silicon_generation(chip: &str) -> Option<u32> {
    let rest = chip.strip_prefix("Apple M")?;
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

/// Picks the model that best balances speed and accuracy for dictation on this machine.
pub fn recommend(device: &DeviceInfo, english_only_ok: bool) -> Recommendation {
    let candidates: Vec<&ModelInfo> = CATALOG
        .iter()
        .filter(|m| english_only_ok || !m.english_only)
        .collect();
    let fits: Vec<&ModelInfo> = candidates
        .iter()
        .copied()
        .filter(|m| m.min_ram_gb <= device.ram_gb)
        .collect();
    let pool = if fits.is_empty() { candidates } else { fits };

    let best = pool
        .into_iter()
        .max_by(|a, b| score(a, device).total_cmp(&score(b, device)))
        .unwrap_or(&CATALOG[0]);

    let reason = format!(
        "Fast and accurate enough for live dictation, and {} on your {}.",
        if best.size_mb < 600 { "loads quickly" } else { "runs comfortably" },
        device.chip
    );
    Recommendation { model_id: best.id, reason }
}

fn score(model: &ModelInfo, device: &DeviceInfo) -> f64 {
    const SPEED_WEIGHT: f64 = 0.45;
    const ACCURACY_WEIGHT: f64 = 0.55;
    let mut score = SPEED_WEIGHT * (model.speed / 10.0) * (0.5 + 0.5 * device.performance_tier)
        + ACCURACY_WEIGHT * (model.accuracy / 10.0)
        + ((device.ram_gb.saturating_sub(model.min_ram_gb)) as f64 * 0.01).min(0.1);
    // Large models are slow without a GPU.
    if !device.gpu && model.size_mb > 1000 {
        score -= 0.15;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(chip: &str, ram_gb: u32, gpu: bool) -> DeviceInfo {
        DeviceInfo {
            chip: chip.into(),
            ram_gb,
            cores: 8,
            gpu,
            summary: String::new(),
            performance_tier: performance_tier(chip, 8, gpu),
        }
    }

    #[test]
    fn reads_apple_silicon_generation() {
        assert_eq!(apple_silicon_generation("Apple M3 Pro"), Some(3));
        assert_eq!(apple_silicon_generation("Apple M10"), Some(10));
        assert_eq!(apple_silicon_generation("Intel(R) Core(TM) i7"), None);
    }

    #[test]
    fn recommends_turbo_on_capable_machines() {
        let rec = recommend(&device("Apple M3", 16, true), true);
        assert_eq!(rec.model_id, "large-v3-turbo-q5");
        let rec = recommend(&device("AMD Ryzen 7", 16, false), true);
        assert_eq!(rec.model_id, "large-v3-turbo-q5");
    }

    #[test]
    fn respects_ram_and_language() {
        let rec = recommend(&device("Intel Celeron", 2, false), true);
        assert!(crate::models::find(rec.model_id).unwrap().min_ram_gb <= 2);
        let rec = recommend(&device("Intel Celeron", 2, false), false);
        assert!(!crate::models::find(rec.model_id).unwrap().english_only);
    }
}
