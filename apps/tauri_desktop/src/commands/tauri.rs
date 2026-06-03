use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::mvp;
use crate::capture::launcher;
use crate::settings::dto::AppSettingsDto;
use crate::settings::{models, store};
use crate::shell_state::ShellState;
use crate::tray;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub boot_summary: String,
    pub model_summary: String,
    pub history_summary: String,
    pub local_ocr_runtime_status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MvpRunResponse {
    pub events: Vec<String>,
    pub history_summary: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    pub events: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportResponse {
    pub events: Vec<String>,
    pub model_summary: String,
    pub models: Vec<ModelSummaryDto>,
    pub settings: AppSettingsDto,
}

#[derive(Debug, Default)]
pub struct ModelDownloadRuntime {
    task: Option<ModelDownloadTask>,
}

#[derive(Debug, Clone)]
struct ModelDownloadTask {
    model_id: String,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<ModelDownloadProgressState>>,
}

#[derive(Debug, Clone)]
struct ModelDownloadProgressState {
    running: bool,
    model_id: String,
    role: String,
    file_name: String,
    file_index: usize,
    file_count: usize,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    error: Option<String>,
    result: Option<ModelImportResponse>,
}

impl ModelDownloadProgressState {
    fn started(model_id: String) -> Self {
        Self {
            running: true,
            model_id,
            role: String::new(),
            file_name: String::new(),
            file_index: 0,
            file_count: 0,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            result: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadStatusDto {
    pub running: bool,
    pub model_id: String,
    pub role: String,
    pub file_name: String,
    pub file_index: usize,
    pub file_count: usize,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f32>,
    pub error: Option<String>,
    pub result: Option<ModelImportResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummaryDto {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub backend: String,
    pub source: String,
    pub availability: String,
    pub path: Option<String>,
    pub package_source: Option<String>,
}

pub type ModelStorageInfoDto = models::ModelStorageInfo;

#[tauri::command]
pub fn app_status(state: State<'_, Mutex<ShellState>>) -> Result<AppStatus, String> {
    log::info!("tauri command app_status started");
    let state = state.lock().map_err(|_| "shell state lock poisoned")?;

    let status = AppStatus {
        boot_summary: state.boot_summary(),
        model_summary: state.model_summary(),
        history_summary: state.history_summary(),
        local_ocr_runtime_status: state.local_ocr_runtime_status().to_owned(),
    };
    log::info!("tauri command app_status completed");
    Ok(status)
}

#[tauri::command]
pub fn get_settings(state: State<'_, Mutex<ShellState>>) -> Result<AppSettingsDto, String> {
    log::info!("tauri command get_settings started");
    let state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let settings = AppSettingsDto::from(state.settings());
    log::info!("tauri command get_settings completed");
    Ok(settings)
}

#[tauri::command]
pub fn list_models(state: State<'_, Mutex<ShellState>>) -> Result<Vec<ModelSummaryDto>, String> {
    log::info!("tauri command list_models started");
    let state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let models = model_summaries(state.model_manifests());
    log::info!("tauri command list_models completed");
    Ok(models)
}

#[tauri::command]
pub fn model_storage_info(app: AppHandle) -> Result<ModelStorageInfoDto, String> {
    log::info!("tauri command model_storage_info started");
    let info = models::storage_info(&app)?;
    log::info!("tauri command model_storage_info completed");
    Ok(info)
}

#[tauri::command]
pub fn choose_ocr_model_storage_dir(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
    runtime: State<'_, Mutex<ModelDownloadRuntime>>,
) -> Result<Option<ModelStorageInfoDto>, String> {
    log::info!("tauri command choose_ocr_model_storage_dir started");
    ensure_no_model_download_running(&runtime)?;
    let Some(path) = platform_win32::prompt_folder_path("Choose OCR model download location")
        .map_err(|error| format!("{}: {}", error.code, error.message))?
    else {
        log::info!("tauri command choose_ocr_model_storage_dir canceled");
        return Ok(None);
    };

    let info = update_ocr_model_storage_dir(&app, &state, path)?;
    log::info!("tauri command choose_ocr_model_storage_dir completed");
    Ok(Some(info))
}

#[tauri::command]
pub fn set_ocr_model_storage_dir(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
    runtime: State<'_, Mutex<ModelDownloadRuntime>>,
    path: String,
) -> Result<ModelStorageInfoDto, String> {
    log::info!("tauri command set_ocr_model_storage_dir started");
    ensure_no_model_download_running(&runtime)?;
    let info = update_ocr_model_storage_dir(&app, &state, PathBuf::from(path))?;
    log::info!("tauri command set_ocr_model_storage_dir completed");
    Ok(info)
}

#[tauri::command]
pub fn open_ocr_model_storage_dir(app: AppHandle) -> Result<(), String> {
    log::info!("tauri command open_ocr_model_storage_dir started");
    let info = models::storage_info(&app)?;
    let path = PathBuf::from(info.current_ocr_models_dir);
    std::fs::create_dir_all(&path).map_err(|error| {
        format!(
            "model_storage_create_failed: failed to create '{}': {error}",
            path.display()
        )
    })?;
    open_folder(&path)?;
    log::info!("tauri command open_ocr_model_storage_dir completed");
    Ok(())
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
    settings: AppSettingsDto,
) -> Result<AppSettingsDto, String> {
    log::info!(
        "tauri command save_settings started language={} capture_hotkey={}",
        settings.interface.language,
        settings.hotkeys.capture
    );
    let mut settings = shared_models::Settings::from(settings);
    let saved = {
        let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
        if should_disable_local_auto_ocr(&settings, &state) {
            log::warn!("disabling local auto OCR because runtime or model is not ready");
            settings.ocr.auto_run_after_capture = false;
        }
        state.update_settings(settings.clone());
        AppSettingsDto::from(state.settings())
    };

    store::save(&app, &saved)?;
    tray::refresh(&app, &saved.interface.language).map_err(|error| error.to_string())?;
    launcher::register_capture_hotkey_for_settings(&app, &settings)?;
    if let Err(error) = launcher::ensure_overlay_resident(&app) {
        log::error!("failed to ensure resident overlay after settings save: {error}");
    }
    log::info!("tauri command save_settings completed");
    Ok(saved)
}

#[tauri::command]
pub fn run_mvp_flow(state: State<'_, Mutex<ShellState>>) -> Result<MvpRunResponse, String> {
    log::info!("tauri command run_mvp_flow started");
    let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let events = mvp::run_mvp_capture_ocr_translate(&mut state)
        .into_iter()
        .map(|event| format!("{event:?}"))
        .collect();

    log::info!("tauri command run_mvp_flow completed");
    Ok(MvpRunResponse {
        events,
        history_summary: state.history_summary(),
    })
}

#[tauri::command]
pub fn drain_events(state: State<'_, Mutex<ShellState>>) -> Result<MvpRunResponse, String> {
    log::info!("tauri command drain_events started");
    let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let events = state
        .dispatch(shared_models::CoreCommand::DrainEvents)
        .into_iter()
        .map(|event| format!("{event:?}"))
        .collect();

    log::info!("tauri command drain_events completed");
    Ok(MvpRunResponse {
        events,
        history_summary: state.history_summary(),
    })
}

#[tauri::command]
pub fn import_model(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
    manifest_path: String,
) -> Result<ModelImportResponse, String> {
    log::info!("tauri command import_model started");
    let manifest = models::storage(&app)?
        .import_manifest_file(&manifest_path)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let (events, model_summary, model_list, settings) = {
        let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
        let events = state
            .dispatch(shared_models::CoreCommand::RegisterModel(manifest))
            .into_iter()
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        models::save(&app, state.model_manifests())?;
        let settings = AppSettingsDto::from(state.settings());
        (
            events,
            state.model_summary(),
            model_summaries(state.model_manifests()),
            settings,
        )
    };

    log::info!("tauri command import_model completed");
    Ok(ModelImportResponse {
        events,
        model_summary,
        models: model_list,
        settings,
    })
}

#[tauri::command]
pub fn download_builtin_ocr_model(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
    model_id: String,
) -> Result<ModelImportResponse, String> {
    log::info!("tauri command download_builtin_ocr_model started model_id={model_id}");
    let source = model_registry::find_builtin_ocr_package_source(&model_id).ok_or_else(|| {
        format!("model_download_source_missing: unsupported model id '{model_id}'")
    })?;
    let manifest = models::storage(&app)?
        .download_builtin_ocr_package(&source)
        .map_err(|error| format!("{}: {}", error.code, error.message))?
        .manifest;

    let (events, model_summary, model_list, settings) = {
        let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
        let events = state
            .dispatch(shared_models::CoreCommand::RegisterModel(manifest.clone()))
            .into_iter()
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        let mut next_settings = state.settings().clone();
        next_settings.ocr.default_model_id = Some(manifest.id.clone());
        state.update_settings(next_settings);
        let settings = AppSettingsDto::from(state.settings());
        models::save(&app, state.model_manifests())?;
        store::save(&app, &settings)?;
        (
            events,
            state.model_summary(),
            model_summaries(state.model_manifests()),
            settings,
        )
    };

    log::info!("tauri command download_builtin_ocr_model completed");
    Ok(ModelImportResponse {
        events,
        model_summary,
        models: model_list,
        settings,
    })
}

#[tauri::command]
pub fn start_builtin_ocr_model_download(
    app: AppHandle,
    runtime: State<'_, Mutex<ModelDownloadRuntime>>,
    model_id: String,
) -> Result<ModelDownloadStatusDto, String> {
    log::info!("tauri command start_builtin_ocr_model_download started model_id={model_id}");
    let source = model_registry::find_builtin_ocr_package_source(&model_id).ok_or_else(|| {
        format!("model_download_source_missing: unsupported model id '{model_id}'")
    })?;
    let progress = Arc::new(Mutex::new(ModelDownloadProgressState::started(
        model_id.clone(),
    )));
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut runtime = runtime
            .lock()
            .map_err(|_| "model download runtime lock poisoned")?;
        if let Some(task) = &runtime.task {
            if task
                .progress
                .lock()
                .map_err(|_| "model download progress lock poisoned")?
                .running
            {
                return Err(format!(
                    "model_download_already_running: '{}' is already downloading",
                    task.model_id
                ));
            }
        }
        runtime.task = Some(ModelDownloadTask {
            model_id: model_id.clone(),
            cancel: cancel.clone(),
            progress: progress.clone(),
        });
    }

    let app_for_thread = app.clone();
    let progress_for_thread = progress.clone();
    thread::spawn(move || {
        let result = download_builtin_ocr_model_in_background(
            app_for_thread,
            source,
            progress_for_thread.clone(),
            cancel,
        );
        match progress_for_thread.lock() {
            Ok(mut progress) => {
                progress.running = false;
                match result {
                    Ok(response) => {
                        progress.result = Some(response);
                        progress.error = None;
                    }
                    Err(error) => {
                        progress.error = Some(error);
                    }
                }
            }
            Err(error) => {
                log::error!("model download progress lock poisoned: {error}");
            }
        }
    });

    model_download_status_from_progress(&progress)
}

#[tauri::command]
pub fn model_download_status(
    runtime: State<'_, Mutex<ModelDownloadRuntime>>,
) -> Result<Option<ModelDownloadStatusDto>, String> {
    let runtime = runtime
        .lock()
        .map_err(|_| "model download runtime lock poisoned")?;
    let Some(task) = &runtime.task else {
        return Ok(None);
    };

    model_download_status_from_progress(&task.progress).map(Some)
}

#[tauri::command]
pub fn cancel_model_download(
    runtime: State<'_, Mutex<ModelDownloadRuntime>>,
) -> Result<Option<ModelDownloadStatusDto>, String> {
    let runtime = runtime
        .lock()
        .map_err(|_| "model download runtime lock poisoned")?;
    let Some(task) = &runtime.task else {
        return Ok(None);
    };
    task.cancel.store(true, Ordering::Relaxed);
    model_download_status_from_progress(&task.progress).map(Some)
}

fn download_builtin_ocr_model_in_background(
    app: AppHandle,
    source: model_registry::ModelPackageSource,
    progress: Arc<Mutex<ModelDownloadProgressState>>,
    cancel: Arc<AtomicBool>,
) -> Result<ModelImportResponse, String> {
    let manifest = models::storage(&app)?
        .download_builtin_ocr_package_with_progress(
            &source,
            |download_progress| {
                if let Ok(mut state) = progress.lock() {
                    state.role = download_progress.role;
                    state.file_name = download_progress.file_name;
                    state.file_index = download_progress.file_index;
                    state.file_count = download_progress.file_count;
                    state.downloaded_bytes = download_progress.downloaded_bytes;
                    state.total_bytes = download_progress.total_bytes;
                }
            },
            || cancel.load(Ordering::Relaxed),
        )
        .map_err(|error| format!("{}: {}", error.code, error.message))?
        .manifest;

    register_downloaded_model(app, manifest)
}

fn register_downloaded_model(
    app: AppHandle,
    manifest: shared_models::ModelManifest,
) -> Result<ModelImportResponse, String> {
    let (events, model_summary, model_list, settings) = {
        let state = app.state::<Mutex<ShellState>>();
        let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
        let events = state
            .dispatch(shared_models::CoreCommand::RegisterModel(manifest.clone()))
            .into_iter()
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        let mut next_settings = state.settings().clone();
        next_settings.ocr.default_model_id = Some(manifest.id.clone());
        state.update_settings(next_settings);
        let settings = AppSettingsDto::from(state.settings());
        models::save(&app, state.model_manifests())?;
        store::save(&app, &settings)?;
        (
            events,
            state.model_summary(),
            model_summaries(state.model_manifests()),
            settings,
        )
    };

    Ok(ModelImportResponse {
        events,
        model_summary,
        models: model_list,
        settings,
    })
}

fn model_download_status_from_progress(
    progress: &Arc<Mutex<ModelDownloadProgressState>>,
) -> Result<ModelDownloadStatusDto, String> {
    let progress = progress
        .lock()
        .map_err(|_| "model download progress lock poisoned")?
        .clone();
    let percent = progress.total_bytes.and_then(|total| {
        (total > 0).then(|| (progress.downloaded_bytes as f32 / total as f32 * 100.0).min(100.0))
    });

    Ok(ModelDownloadStatusDto {
        running: progress.running,
        model_id: progress.model_id,
        role: progress.role,
        file_name: progress.file_name,
        file_index: progress.file_index,
        file_count: progress.file_count,
        downloaded_bytes: progress.downloaded_bytes,
        total_bytes: progress.total_bytes,
        percent,
        error: progress.error,
        result: progress.result,
    })
}

fn ensure_no_model_download_running(
    runtime: &State<'_, Mutex<ModelDownloadRuntime>>,
) -> Result<(), String> {
    let runtime = runtime
        .lock()
        .map_err(|_| "model download runtime lock poisoned")?;
    let Some(task) = &runtime.task else {
        return Ok(());
    };
    if task
        .progress
        .lock()
        .map_err(|_| "model download progress lock poisoned")?
        .running
    {
        return Err("model_storage_busy: wait for the current model download to finish".to_owned());
    }
    Ok(())
}

fn update_ocr_model_storage_dir(
    app: &AppHandle,
    state: &State<'_, Mutex<ShellState>>,
    path: PathBuf,
) -> Result<ModelStorageInfoDto, String> {
    let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let info = models::set_ocr_models_dir(app, state.model_manifests_mut(), path)?;
    Ok(info)
}

fn open_folder(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|error| {
                format!(
                    "model_storage_open_failed: failed to open '{}': {error}",
                    path.display()
                )
            })?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        Err(format!(
            "unsupported_platform: opening folders is currently implemented only on Windows ({})",
            path.display()
        ))
    }
}

fn model_summaries(models: &[shared_models::ModelManifest]) -> Vec<ModelSummaryDto> {
    models
        .iter()
        .map(|model| {
            let (source, availability, path) = model_source_status(model);
            ModelSummaryDto {
                id: model.id.clone(),
                name: model.name.clone(),
                domain: match model.domain {
                    shared_models::ModelDomain::Ocr => "ocr",
                    shared_models::ModelDomain::Translation => "translation",
                }
                .to_owned(),
                backend: model.backend.clone(),
                source,
                availability,
                path,
                package_source: model_registry::find_builtin_ocr_package_source(&model.id)
                    .map(|source| source.source_name.to_owned()),
            }
        })
        .collect()
}

fn model_source_status(model: &shared_models::ModelManifest) -> (String, String, Option<String>) {
    match &model.source {
        shared_models::ModelSource::BuiltIn => {
            ("built-in".to_owned(), "manifest-only".to_owned(), None)
        }
        shared_models::ModelSource::Download { url, .. } => (
            "download".to_owned(),
            "not-downloaded".to_owned(),
            Some(url.clone()),
        ),
        shared_models::ModelSource::LocalPath(root) => {
            let missing = model
                .files
                .iter()
                .filter(|file| file.required)
                .any(|file| !Path::new(root).join(&file.path).exists());
            (
                "local-path".to_owned(),
                if missing { "missing-files" } else { "ready" }.to_owned(),
                Some(root.clone()),
            )
        }
    }
}

fn should_disable_local_auto_ocr(settings: &shared_models::Settings, state: &ShellState) -> bool {
    if !settings.ocr.auto_run_after_capture {
        return false;
    }
    if !matches!(settings.ocr.provider, shared_models::OcrProvider::Local(_)) {
        return false;
    }
    if state.local_ocr_runtime_status() != "local-ocr-rs-enabled" {
        return true;
    }

    let selected_model_id = settings.ocr.default_model_id.as_deref();
    !has_ready_local_ocr_model(state.model_manifests(), selected_model_id)
}

fn has_ready_local_ocr_model(
    models: &[shared_models::ModelManifest],
    selected_model_id: Option<&str>,
) -> bool {
    models.iter().any(|model| {
        if selected_model_id.is_some_and(|id| id != model.id) {
            return false;
        }
        if model.domain != shared_models::ModelDomain::Ocr || model.backend != "mnn" {
            return false;
        }
        let shared_models::ModelSource::LocalPath(root) = &model.source else {
            return false;
        };
        model
            .files
            .iter()
            .filter(|file| file.required)
            .all(|file| Path::new(root).join(&file.path).exists())
    })
}

#[tauri::command]
pub fn start_capture(
    app: AppHandle,
    state: State<'_, Mutex<ShellState>>,
) -> Result<CaptureResponse, String> {
    log::info!("tauri command start_capture started");
    let mut state = state.lock().map_err(|_| "shell state lock poisoned")?;
    let events = mvp::start_capture(&mut state)
        .into_iter()
        .map(|event| format!("{event:?}"))
        .collect();
    drop(state);

    launcher::launch_capture_overlay(&app)?;

    log::info!("tauri command start_capture completed");
    Ok(CaptureResponse { events })
}
