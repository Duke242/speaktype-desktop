mod audio;
mod commands;
mod device;
mod dictation;
mod engine;
mod history;
mod media;
mod models;
mod paste;
mod pill;
mod pipeline;
mod platform;
mod settings;
mod text;
mod tray;

use std::sync::{Arc, Mutex, OnceLock, RwLock};

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use crate::{
    device::DeviceInfo,
    dictation::{Controller, DictationState, Event},
    history::History,
    models::{ModelInfo, ModelStore},
    settings::{Settings, SettingsStore},
    engine::Engine,
};

pub use tray::TRAY_ID;

/// Benchmarks for development (`cargo run --release --example transcribe_wav`).
#[doc(hidden)]
pub mod devtools {
    use std::{path::Path, time::Instant};

    /// Downloads a catalog model into `models_dir`, printing progress.
    pub fn download(model_id: &str, models_dir: &Path, accelerator: Option<bool>) -> Result<(), String> {
        let store = crate::models::ModelStore::new(models_dir.to_path_buf());
        tauri::async_runtime::block_on(store.download(model_id, accelerator, |p| {
            if p.total > 0 {
                println!("  {}: {:.0}%", p.id, p.downloaded as f64 / p.total as f64 * 100.0);
            }
        }))?;
        let status = store.statuses().into_iter().find(|s| s.info.id == model_id).ok_or("Unknown model id")?;
        println!("downloaded={} accelerator={:?}", status.downloaded, status.accelerator);
        Ok(())
    }

    /// Loads a catalog model from `models_dir` and transcribes a WAV file
    /// `runs` times, printing load and transcription times.
    pub fn benchmark(model_id: &str, models_dir: &Path, wav: &Path, runs: usize) -> Result<(), String> {
        let model = crate::models::find(model_id).ok_or("Unknown model id")?;
        let store = crate::models::ModelStore::new(models_dir.to_path_buf());
        let (samples, duration) = crate::media::decode_file(wav)?;

        crate::engine::silence_logs();
        let mut engine = crate::engine::Engine::default();
        let started = Instant::now();
        engine.load(model, &store.path(model))?;
        println!("{model_id}: loaded in {:.2?}", started.elapsed());

        for run in 1..=runs {
            let started = Instant::now();
            let text = engine.transcribe(&samples, "auto")?;
            let elapsed = started.elapsed();
            println!(
                "  run {run}: {duration:.1}s of audio in {elapsed:.2?} ({:.0}x real time)",
                duration / elapsed.as_secs_f64()
            );
            if run == runs {
                println!("  text: {}", text.trim());
            }
        }
        Ok(())
    }
}

pub struct AppState {
    settings: RwLock<Settings>,
    settings_store: SettingsStore,
    pub models: ModelStore,
    pub engine: Arc<Mutex<Engine>>,
    /// The model currently being loaded into memory, if any.
    pub engine_loading: Mutex<Option<String>>,
    pub history: Mutex<History>,
    pub controller: Controller,
    pub dictation_state: Mutex<DictationState>,
    /// Set when the saved hotkey couldn't be registered at launch.
    pub hotkey_error: Mutex<Option<String>>,
    device: OnceLock<DeviceInfo>,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn replace_settings(&self, settings: Settings) -> Result<(), String> {
        self.settings_store.save(&settings)?;
        *self.settings.write().unwrap() = settings;
        Ok(())
    }

    /// Detected once; reading the CPU and memory takes a moment.
    pub fn device(&self) -> &DeviceInfo {
        self.device.get_or_init(device::detect)
    }

    /// Loads `model` into `engine`, telling the UI while it happens.
    pub fn load_model(
        &self,
        app: &AppHandle,
        engine: &mut Engine,
        model: &ModelInfo,
    ) -> Result<(), String> {
        if engine.loaded_model() == Some(model.id) {
            return Ok(());
        }
        *self.engine_loading.lock().unwrap() = Some(model.id.to_string());
        let _ = app.emit("engine-changed", ());
        let result = engine.load(model, &self.models.path(model));
        *self.engine_loading.lock().unwrap() = None;
        let _ = app.emit("engine-changed", ());
        result
    }

    /// Loads a downloaded model in the background so the first dictation is fast.
    pub fn warm_up(&self, app: &AppHandle, id: &str) {
        let Some(model) = models::find(id) else {
            return;
        };
        if !self.models.is_downloaded(id) {
            return;
        }
        let app = app.clone();
        std::thread::spawn(move || {
            let state = app.state::<AppState>();
            let mut engine = state.engine.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(e) = state.load_model(&app, &mut engine, model) {
                eprintln!("[models] warm-up failed: {e}");
            }
        });
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            engine::silence_logs();
            let settings_store = SettingsStore::new(app.path().app_config_dir()?);
            let settings = settings_store.load();
            let data_dir = app.path().app_data_dir()?;

            app.manage(AppState {
                settings: RwLock::new(settings.clone()),
                settings_store,
                models: ModelStore::new(data_dir.join("models")),
                engine: Arc::default(),
                engine_loading: Mutex::new(None),
                history: Mutex::new(History::load(&data_dir)),
                controller: Controller::spawn(app.handle().clone()),
                dictation_state: Mutex::new(DictationState::Idle),
                hotkey_error: Mutex::new(None),
                device: OnceLock::new(),
            });

            if let Some(main) = app.get_webview_window("main") {
                platform::style_main_window(&main);
            }
            pill::set_interactive(app.handle(), false);

            // Shortcut registration waits on the main thread, so it can't run
            // here before the event loop starts.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let state = handle.state::<AppState>();
                let result = state
                    .controller
                    .register_hotkey(&handle, &settings.hotkey, None);
                if let Err(e) = result {
                    eprintln!("[hotkey] {e}");
                    *state.hotkey_error.lock().unwrap() = Some(e);
                }
                state.controller.send(Event::RefreshPill);
                state.warm_up(&handle, &settings.selected_model);
                state.device();
            });

            tray::build(app.handle(), settings.show_tray_icon)?;
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // Closing the main window keeps SpeakType running in the tray.
            ("main", WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                let _ = window.hide();
            }
            // The menu bar panel behaves like a popover: it closes when you click elsewhere.
            (tray::PANEL_LABEL, WindowEvent::Focused(false)) => tray::panel_blurred(window.app_handle()),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::save_settings,
            commands::list_input_devices,
            commands::list_models,
            commands::get_engine_status,
            commands::get_device_info,
            commands::download_model,
            commands::cancel_download,
            commands::delete_model,
            commands::get_history,
            commands::get_stats,
            commands::delete_history_item,
            commands::clear_history,
            commands::read_history_audio,
            commands::reveal_history_audio,
            commands::toggle_dictation,
            commands::get_dictation_state,
            commands::transcribe_file,
            commands::get_permissions,
            commands::request_permission,
            commands::open_permission_settings,
            commands::check_for_update,
            commands::open_main_window,
            commands::hide_tray_panel,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building SpeakType");

    app.run(|app, event| {
        // macOS: clicking the Dock icon brings the hidden window back.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Reopen { .. } = event {
            tray::open_main_window(app, None);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
