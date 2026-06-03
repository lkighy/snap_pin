use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

// Resolves the overlay binary in packaged builds and falls back to cargo during workspace runs.
pub(crate) enum OverlayLaunch {
    Executable(PathBuf),
    Cargo { workspace: PathBuf },
}

impl OverlayLaunch {
    pub(crate) fn description(&self) -> String {
        match self {
            OverlayLaunch::Executable(path) => format!("executable {}", path.display()),
            OverlayLaunch::Cargo { workspace } => format!("cargo run in {}", workspace.display()),
        }
    }

    pub(crate) fn command(&self, overlay_args: Vec<String>) -> std::process::Command {
        match self {
            OverlayLaunch::Executable(path) => {
                let mut command = std::process::Command::new(path);
                command.args(overlay_args);
                command
            }
            OverlayLaunch::Cargo { workspace } => {
                let mut command = std::process::Command::new("cargo");
                command
                    .arg("run")
                    .arg("-p")
                    .arg("egui_overlay")
                    .arg("--features")
                    .arg("local-ocr-rs")
                    .arg("--")
                    .args(overlay_args)
                    .current_dir(workspace);
                prepend_mnn_dll_path(&mut command, workspace);
                command
            }
        }
    }
}

fn prepend_mnn_dll_path(command: &mut std::process::Command, workspace: &Path) {
    let mnn_lib = workspace
        .join("third_party")
        .join("ocr-rs-2.2.2")
        .join("3rd_party")
        .join("prebuilt")
        .join("mnn-dev-windows-x86_64")
        .join("lib");

    if !mnn_lib.exists() {
        return;
    }

    let path = match std::env::var_os("PATH") {
        Some(current) => {
            let mut paths = vec![mnn_lib];
            paths.extend(std::env::split_paths(&current));
            std::env::join_paths(paths).ok()
        }
        None => Some(mnn_lib.into_os_string()),
    };

    if let Some(path) = path {
        command.env("PATH", path);
    }
}

pub(crate) fn overlay_launch(app: &AppHandle) -> Result<OverlayLaunch, String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let file_name = executable_name("egui_overlay");

    if let Some(workspace) = find_workspace_root(&current_exe) {
        if is_workspace_debug_executable(&current_exe, &workspace) {
            log::info!(
                "using cargo overlay launch current_exe={} workspace={}",
                current_exe.display(),
                workspace.display()
            );
            return Ok(OverlayLaunch::Cargo { workspace });
        }
    }

    for directory in candidate_directories(app, &current_exe) {
        let candidate = directory.join(&file_name);
        if candidate.exists() {
            log::info!("using overlay executable {}", candidate.display());
            return Ok(OverlayLaunch::Executable(candidate));
        }
    }

    if let Some(workspace) = find_workspace_root(&current_exe) {
        log::info!(
            "falling back to cargo overlay launch workspace={}",
            workspace.display()
        );
        return Ok(OverlayLaunch::Cargo { workspace });
    }

    Ok(OverlayLaunch::Executable(
        current_exe.with_file_name(file_name),
    ))
}

fn is_workspace_debug_executable(current_exe: &Path, workspace: &Path) -> bool {
    current_exe.starts_with(workspace.join("target").join("debug"))
}

fn candidate_directories(app: &AppHandle, current_exe: &PathBuf) -> Vec<PathBuf> {
    let mut directories = Vec::new();

    if let Some(parent) = current_exe.parent() {
        directories.push(parent.to_path_buf());
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        directories.push(resource_dir);
    }

    directories
}

fn executable_name(name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{name}.exe")
    }

    #[cfg(not(windows))]
    {
        name.to_owned()
    }
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.parent();
    while let Some(directory) = current {
        let manifest = directory.join("Cargo.toml");
        let apps_dir = directory.join("apps");
        if manifest.exists() && apps_dir.exists() {
            return Some(directory.to_path_buf());
        }

        current = directory.parent();
    }

    std::env::current_dir().ok().and_then(|cwd| {
        if cwd.join("Cargo.toml").exists() && cwd.join("apps").exists() {
            Some(cwd)
        } else {
            None
        }
    })
}
