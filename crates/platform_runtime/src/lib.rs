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

#[cfg(windows)]
fn create_target_platform() -> Box<dyn AppPlatform> {
    Box::<platform_win32::Win32Platform>::default()
}

#[cfg(windows)]
fn create_target_platform_arc() -> Arc<dyn AppPlatform> {
    Arc::<platform_win32::Win32Platform>::default()
}

#[cfg(not(windows))]
fn create_target_platform() -> Box<dyn AppPlatform> {
    Box::<stub::StubPlatform>::default()
}

#[cfg(not(windows))]
fn create_target_platform_arc() -> Arc<dyn AppPlatform> {
    Arc::<stub::StubPlatform>::default()
}
