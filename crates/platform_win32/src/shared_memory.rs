#[cfg(windows)]
pub use win32_mapping::{NamedSharedMemory, create_named_shared_memory, read_named_shared_memory};

#[cfg(not(windows))]
use crate::PlatformError;

#[cfg(not(windows))]
pub struct NamedSharedMemory;

#[cfg(not(windows))]
pub fn create_named_shared_memory(
    _name: &str,
    _bytes: &[u8],
) -> Result<NamedSharedMemory, PlatformError> {
    Err(PlatformError::new(
        "unsupported_platform",
        "shared screenshot memory is currently implemented only on Windows",
    ))
}

#[cfg(not(windows))]
pub fn read_named_shared_memory(_name: &str, _len: usize) -> Result<Vec<u8>, PlatformError> {
    Err(PlatformError::new(
        "unsupported_platform",
        "shared screenshot memory is currently implemented only on Windows",
    ))
}

#[cfg(windows)]
mod win32_mapping {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::copy_nonoverlapping;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS,
        MapViewOfFile, OpenFileMappingW, PAGE_READWRITE, UnmapViewOfFile,
    };

    use crate::PlatformError;

    #[derive(Debug)]
    pub struct NamedSharedMemory {
        handle: HANDLE,
        view: *mut u8,
        len: usize,
    }

    // The mapping owns a process-local handle and view. We only move it between
    // threads to keep the object alive; no shared mutable access is exposed.
    unsafe impl Send for NamedSharedMemory {}

    pub fn create_named_shared_memory(
        name: &str,
        bytes: &[u8],
    ) -> Result<NamedSharedMemory, PlatformError> {
        if bytes.is_empty() {
            return Err(PlatformError::new(
                "empty_shared_memory",
                "shared screenshot memory cannot be empty",
            ));
        }

        let wide_name = wide_null(name);
        let len = bytes.len();
        let max_size = len as u64;
        let high = (max_size >> 32) as u32;
        let low = max_size as u32;

        // SAFETY: INVALID_HANDLE_VALUE creates a page-file backed mapping. The
        // name is nul-terminated and valid for the duration of the call.
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                std::ptr::null(),
                PAGE_READWRITE,
                high,
                low,
                wide_name.as_ptr(),
            )
        };
        if handle.is_null() {
            return Err(PlatformError::new(
                "shared_memory_create_failed",
                "failed to create the screenshot memory mapping",
            ));
        }

        // SAFETY: handle is a valid file mapping object and len is non-zero.
        let mapped = unsafe { MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, len) };
        let view = mapped.Value as *mut u8;
        if view.is_null() {
            // SAFETY: handle was returned by CreateFileMappingW above.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(PlatformError::new(
                "shared_memory_map_failed",
                "failed to map the screenshot memory for writing",
            ));
        }

        // SAFETY: view points to len writable bytes and bytes has len readable bytes.
        unsafe {
            copy_nonoverlapping(bytes.as_ptr(), view, len);
        }

        Ok(NamedSharedMemory { handle, view, len })
    }

    pub fn read_named_shared_memory(name: &str, len: usize) -> Result<Vec<u8>, PlatformError> {
        if len == 0 {
            return Err(PlatformError::new(
                "empty_shared_memory",
                "shared screenshot memory cannot be empty",
            ));
        }

        let wide_name = wide_null(name);
        // SAFETY: the name is nul-terminated and valid for the duration of the call.
        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, 0, wide_name.as_ptr()) };
        if handle.is_null() {
            return Err(PlatformError::new(
                "shared_memory_open_failed",
                "failed to open the screenshot memory mapping",
            ));
        }

        // SAFETY: handle is a valid file mapping object and len is non-zero.
        let mapped = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, len) };
        let view = mapped.Value as *const u8;
        if view.is_null() {
            // SAFETY: handle was returned by OpenFileMappingW above.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(PlatformError::new(
                "shared_memory_map_failed",
                "failed to map the screenshot memory for reading",
            ));
        }

        let mut bytes = vec![0; len];
        // SAFETY: view points to len readable bytes and bytes has len writable bytes.
        unsafe {
            copy_nonoverlapping(view, bytes.as_mut_ptr(), len);
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: view.cast_mut().cast(),
            });
            let _ = CloseHandle(handle);
        }

        Ok(bytes)
    }

    impl Drop for NamedSharedMemory {
        fn drop(&mut self) {
            // SAFETY: both resources are owned by this RAII wrapper.
            unsafe {
                if !self.view.is_null() {
                    let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                        Value: self.view.cast(),
                    });
                }
                if !self.handle.is_null() {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }

    impl NamedSharedMemory {
        pub fn len(&self) -> usize {
            self.len
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}
