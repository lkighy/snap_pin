use std::sync::Arc;

use platform_api::AppPlatform;

#[cfg(not(windows))]
mod stub;

pub fn create_platform() -> Box<dyn AppPlatform> {
    create_target_platform()
}

pub fn create_platform_arc() -> Arc<dyn AppPlatform> {
    create_target_platform_arc()
}

pub fn owner_process_is_alive(pid: u32) -> bool {
    target_owner_process_is_alive(pid)
}

#[cfg(windows)]
fn create_target_platform() -> Box<dyn AppPlatform> {
    Box::<platform_win32::Win32Platform>::default()
}

#[cfg(windows)]
fn create_target_platform_arc() -> Arc<dyn AppPlatform> {
    Arc::<platform_win32::Win32Platform>::default()
}

#[cfg(windows)]
fn target_owner_process_is_alive(pid: u32) -> bool {
    platform_win32::process_is_alive(pid)
}

#[cfg(not(windows))]
fn create_target_platform() -> Box<dyn AppPlatform> {
    Box::<stub::StubPlatform>::default()
}

#[cfg(not(windows))]
fn create_target_platform_arc() -> Arc<dyn AppPlatform> {
    Arc::<stub::StubPlatform>::default()
}

#[cfg(not(windows))]
fn target_owner_process_is_alive(_pid: u32) -> bool {
    true
}
