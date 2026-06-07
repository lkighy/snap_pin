use std::mem::{size_of, zeroed};

use shared_models::{ImageFormat, Point, Rect, Size};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BitBlt, CAPTUREBLT, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HBITMAP, HDC, HGDIOBJ, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

use super::CapturedFrame;
use crate::PlatformError;

pub fn virtual_screen_bounds() -> Rect {
    // SAFETY: GetSystemMetrics is pure for these indexes and requires no owned resources.
    let (x, y, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };

    Rect::new(
        Point::new(x as f32, y as f32),
        Size::new(width.max(1) as f32, height.max(1) as f32),
    )
}

pub fn capture_region(region: Option<Rect>) -> Result<CapturedFrame, PlatformError> {
    let rect = normalize_region(region.unwrap_or_else(virtual_screen_bounds))?;
    let width = rect.size.width.round() as i32;
    let height = rect.size.height.round() as i32;
    let x = rect.origin.x.round() as i32;
    let y = rect.origin.y.round() as i32;

    let screen_dc = DeviceContext::screen()?;
    let memory_dc = DeviceContext::compatible(screen_dc.0)?;
    let bitmap = Bitmap::compatible(screen_dc.0, width, height)?;
    let selected = SelectedObject::new(memory_dc.0, bitmap.0)?;

    // SAFETY: all HDC/HBITMAP handles are valid for this scope, and the
    // destination bitmap was selected into the compatible memory DC.
    let copied = unsafe {
        BitBlt(
            memory_dc.0,
            0,
            0,
            width,
            height,
            screen_dc.0,
            x,
            y,
            SRCCOPY | CAPTUREBLT,
        )
    };

    if copied == 0 {
        return Err(PlatformError::new(
            "capture_failed",
            "BitBlt failed while capturing the selected region",
        ));
    }

    drop(selected);
    let bytes = bitmap.read_bgra(memory_dc.0, width, height)?;

    Ok(CapturedFrame {
        pixel_size: Size::new(width as f32, height as f32),
        scale_factor: 1.0,
        format: ImageFormat::Bgra8,
        bytes,
    })
}

fn normalize_region(region: Rect) -> Result<Rect, PlatformError> {
    let width = region.size.width.round();
    let height = region.size.height.round();
    if width < 1.0 || height < 1.0 {
        return Err(PlatformError::new(
            "empty_capture_region",
            "selected capture region is empty",
        ));
    }

    Ok(Rect::new(
        Point::new(region.origin.x.round(), region.origin.y.round()),
        Size::new(width, height),
    ))
}

struct DeviceContext(HDC, DeviceContextKind);

enum DeviceContextKind {
    Screen,
    Memory,
}

impl DeviceContext {
    fn screen() -> Result<Self, PlatformError> {
        // SAFETY: a null HWND requests the desktop DC and must be released with ReleaseDC.
        let dc = unsafe { GetDC(std::ptr::null_mut::<std::ffi::c_void>() as HWND) };
        if dc.is_null() {
            return Err(PlatformError::new(
                "capture_failed",
                "failed to acquire the desktop device context",
            ));
        }

        Ok(Self(dc, DeviceContextKind::Screen))
    }

    fn compatible(source: HDC) -> Result<Self, PlatformError> {
        // SAFETY: source is a valid HDC owned by this capture scope.
        let dc = unsafe { CreateCompatibleDC(source) };
        if dc.is_null() {
            return Err(PlatformError::new(
                "capture_failed",
                "failed to create a compatible memory device context",
            ));
        }

        Ok(Self(dc, DeviceContextKind::Memory))
    }
}

impl Drop for DeviceContext {
    fn drop(&mut self) {
        // SAFETY: the handle kind tracks the correct release function.
        unsafe {
            match self.1 {
                DeviceContextKind::Screen => {
                    let _ = ReleaseDC(std::ptr::null_mut::<std::ffi::c_void>() as HWND, self.0);
                }
                DeviceContextKind::Memory => {
                    let _ = DeleteDC(self.0);
                }
            }
        }
    }
}

struct Bitmap(HBITMAP);

impl Bitmap {
    fn compatible(source: HDC, width: i32, height: i32) -> Result<Self, PlatformError> {
        // SAFETY: source is a valid HDC, and width/height were normalized positive.
        let bitmap = unsafe { CreateCompatibleBitmap(source, width, height) };
        if bitmap.is_null() {
            return Err(PlatformError::new(
                "capture_failed",
                "failed to create a compatible bitmap",
            ));
        }

        Ok(Self(bitmap))
    }

    fn read_bgra(&self, dc: HDC, width: i32, height: i32) -> Result<Vec<u8>, PlatformError> {
        let stride = width as usize * 4;
        let mut bytes = vec![0; stride * height as usize];
        let mut info: BITMAPINFO = unsafe { zeroed() };
        info.bmiHeader.biSize =
            size_of::<windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        info.bmiHeader.biSizeImage = bytes.len() as u32;

        // SAFETY: bytes and info point to valid writable buffers. A negative
        // height requests top-down rows, matching egui's image layout.
        let lines = unsafe {
            GetDIBits(
                dc,
                self.0,
                0,
                height as u32,
                bytes.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        };

        if lines == 0 {
            return Err(PlatformError::new(
                "capture_failed",
                "failed to read pixels from the captured bitmap",
            ));
        }

        Ok(bytes)
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: bitmap was allocated by CreateCompatibleBitmap.
        unsafe {
            let _ = DeleteObject(self.0 as HGDIOBJ);
        }
    }
}

struct SelectedObject {
    dc: HDC,
    previous: HGDIOBJ,
}

impl SelectedObject {
    fn new(dc: HDC, bitmap: HBITMAP) -> Result<Self, PlatformError> {
        // SAFETY: dc and bitmap are valid and owned by the current capture scope.
        let previous = unsafe { SelectObject(dc, bitmap as HGDIOBJ) };
        if previous.is_null() {
            return Err(PlatformError::new(
                "capture_failed",
                "failed to select the capture bitmap into the memory device context",
            ));
        }

        Ok(Self { dc, previous })
    }
}

impl Drop for SelectedObject {
    fn drop(&mut self) {
        // SAFETY: previous was returned by SelectObject for this DC.
        unsafe {
            let _ = SelectObject(self.dc, self.previous);
        }
    }
}
