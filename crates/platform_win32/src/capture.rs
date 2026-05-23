use shared_models::{ImageFormat, Rect, Size};

use crate::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    Dxgi,
    Gdi,
    Wgc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureRequest {
    pub region: Option<Rect>,
    pub include_cursor: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedFrame {
    pub pixel_size: Size,
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

pub trait WindowsCaptureBackend {
    fn kind(&self) -> CaptureBackendKind;
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WgcCaptureBackend;

impl WindowsCaptureBackend for WgcCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        CaptureBackendKind::Wgc
    }

    fn capture(&self, _request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        Err(PlatformError::new(
            "not_implemented",
            "WGC capture backend is reserved for the Windows implementation phase",
        ))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DxgiCaptureBackend;

impl WindowsCaptureBackend for DxgiCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        CaptureBackendKind::Dxgi
    }

    #[cfg(windows)]
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        win32_dxgi::capture_region(request.region)
    }

    #[cfg(not(windows))]
    fn capture(&self, _request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        Err(PlatformError::new(
            "unsupported_platform",
            "DXGI capture backend is available only on Windows",
        ))
    }
}

#[cfg(windows)]
pub fn virtual_screen_bounds() -> Rect {
    win32_gdi::virtual_screen_bounds()
}

#[cfg(not(windows))]
pub fn virtual_screen_bounds() -> Rect {
    Rect::new(shared_models::Point::ZERO, Size::new(1280.0, 720.0))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GdiCaptureBackend;

impl WindowsCaptureBackend for GdiCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        CaptureBackendKind::Gdi
    }

    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        capture_region(request.region)
    }
}

#[cfg(windows)]
pub fn capture_region(region: Option<Rect>) -> Result<CapturedFrame, PlatformError> {
    win32_gdi::capture_region(region)
}

#[cfg(not(windows))]
pub fn capture_region(_region: Option<Rect>) -> Result<CapturedFrame, PlatformError> {
    Err(PlatformError::new(
        "unsupported_platform",
        "screen capture is currently implemented only on Windows",
    ))
}

#[cfg(windows)]
mod win32_gdi {
    use std::mem::{size_of, zeroed};

    use shared_models::{ImageFormat, Point, Rect, Size};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BitBlt, CAPTUREBLT, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, HBITMAP, HDC, HGDIOBJ, ReleaseDC,
        SRCCOPY, SelectObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN,
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
}

#[cfg(windows)]
mod win32_dxgi {
    use std::mem::zeroed;
    use std::slice;

    use shared_models::{ImageFormat, Rect, Size};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
        D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED,
        DXGI_SAMPLE_DESC,
    };
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIAdapter1,
        IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
    };
    use windows::core::{Error as WindowsError, Interface};

    use super::CapturedFrame;
    use crate::PlatformError;

    const FRAME_ATTEMPTS: usize = 3;
    const FRAME_TIMEOUT_MS: u32 = 16;

    pub fn capture_region(region: Option<Rect>) -> Result<CapturedFrame, PlatformError> {
        let requested = normalize_region(region.unwrap_or_else(super::virtual_screen_bounds))?;
        let outputs = enumerate_outputs()?;
        if outputs.is_empty() {
            return Err(PlatformError::new(
                "dxgi_no_outputs",
                "DXGI did not report any attached desktop outputs",
            ));
        }

        let mut canvas = vec![0; requested.width as usize * requested.height as usize * 4];
        let mut last_error = None;
        let mut needed_outputs = 0;
        let mut copied_outputs = 0;

        for output in outputs {
            let Some(intersection) = intersect(requested, output.bounds) else {
                continue;
            };

            needed_outputs += 1;
            match capture_output(&output, intersection, requested, &mut canvas) {
                Ok(()) => copied_outputs += 1,
                Err(error) => last_error = Some(error),
            }
        }

        if copied_outputs != needed_outputs {
            return Err(last_error.unwrap_or_else(|| {
                PlatformError::new(
                    "dxgi_capture_empty",
                    "DXGI could not capture any part of the requested region",
                )
            }));
        }

        if looks_like_empty_frame(&canvas) {
            return Err(PlatformError::new(
                "dxgi_empty_frame",
                "DXGI returned an empty desktop frame",
            ));
        }

        Ok(CapturedFrame {
            pixel_size: Size::new(requested.width as f32, requested.height as f32),
            format: ImageFormat::Bgra8,
            bytes: canvas,
        })
    }

    fn enumerate_outputs() -> Result<Vec<DxgiOutput>, PlatformError> {
        // SAFETY: CreateDXGIFactory1 initializes a COM factory object owned by the returned wrapper.
        let factory: IDXGIFactory1 = unsafe {
            CreateDXGIFactory1().map_err(|error| platform_error("dxgi_factory_failed", error))?
        };
        let mut outputs = Vec::new();
        let mut adapter_index = 0;

        while let Ok(adapter) = unsafe { factory.EnumAdapters1(adapter_index) } {
            let mut output_index = 0;
            while let Ok(output) = unsafe { adapter.EnumOutputs(output_index) } {
                let output1 = output
                    .cast::<IDXGIOutput1>()
                    .map_err(|error| platform_error("dxgi_output_cast_failed", error))?;
                let desc = unsafe {
                    output
                        .GetDesc()
                        .map_err(|error| platform_error("dxgi_output_desc_failed", error))?
                };

                if desc.AttachedToDesktop.as_bool()
                    && matches!(
                        desc.Rotation,
                        DXGI_MODE_ROTATION_UNSPECIFIED | DXGI_MODE_ROTATION_IDENTITY
                    )
                {
                    let rect = desc.DesktopCoordinates;
                    outputs.push(DxgiOutput {
                        adapter: adapter.clone(),
                        output: output1,
                        bounds: PixelRect {
                            x: rect.left,
                            y: rect.top,
                            width: (rect.right - rect.left).max(1),
                            height: (rect.bottom - rect.top).max(1),
                        },
                    });
                }

                output_index += 1;
            }
            adapter_index += 1;
        }

        Ok(outputs)
    }

    fn capture_output(
        output: &DxgiOutput,
        intersection: PixelRect,
        requested: PixelRect,
        canvas: &mut [u8],
    ) -> Result<(), PlatformError> {
        let (device, context) = create_device(&output.adapter)?;
        let duplication = unsafe {
            output
                .output
                .DuplicateOutput(&device)
                .map_err(|error| platform_error("dxgi_duplicate_output_failed", error))?
        };
        let frame = AcquiredFrame::acquire(&duplication)?;
        let desktop_resource = frame.resource.as_ref().ok_or_else(|| {
            PlatformError::new(
                "dxgi_frame_missing",
                "DXGI did not return a desktop frame resource",
            )
        })?;
        let texture = desktop_resource
            .cast::<ID3D11Texture2D>()
            .map_err(|error| platform_error("dxgi_texture_cast_failed", error))?;
        let staging = create_staging_texture(&device, &texture)?;
        let staging_resource = staging
            .cast::<ID3D11Resource>()
            .map_err(|error| platform_error("dxgi_resource_cast_failed", error))?;
        let texture_resource = texture
            .cast::<ID3D11Resource>()
            .map_err(|error| platform_error("dxgi_resource_cast_failed", error))?;

        unsafe {
            context.CopyResource(&staging_resource, &texture_resource);
        }

        let mapped = MappedTexture::map(&context, &staging_resource)?;
        copy_intersection_from_output(
            output.bounds,
            intersection,
            requested,
            mapped.data,
            mapped.row_pitch,
            canvas,
        )
    }

    fn create_device(
        adapter: &IDXGIAdapter1,
    ) -> Result<(ID3D11Device, ID3D11DeviceContext), PlatformError> {
        let feature_levels = [D3D_FEATURE_LEVEL_11_0];
        let mut device = None;
        let mut context = None;
        let mut selected_feature_level = D3D_FEATURE_LEVEL::default();

        unsafe {
            D3D11CreateDevice(
                adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut selected_feature_level),
                Some(&mut context),
            )
            .map_err(|error| platform_error("dxgi_device_create_failed", error))?;
        }

        let device = device.ok_or_else(|| {
            PlatformError::new("dxgi_device_missing", "D3D11 did not return a device")
        })?;
        let context = context.ok_or_else(|| {
            PlatformError::new(
                "dxgi_context_missing",
                "D3D11 did not return a device context",
            )
        })?;

        Ok((device, context))
    }

    fn create_staging_texture(
        device: &ID3D11Device,
        source: &ID3D11Texture2D,
    ) -> Result<ID3D11Texture2D, PlatformError> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            source.GetDesc(&mut desc);
        }
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        desc.MipLevels = 1;
        desc.ArraySize = 1;
        desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        desc.SampleDesc = DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        };

        let mut staging = None;
        unsafe {
            device
                .CreateTexture2D(&desc, None, Some(&mut staging))
                .map_err(|error| platform_error("dxgi_staging_create_failed", error))?;
        }

        staging.ok_or_else(|| {
            PlatformError::new(
                "dxgi_staging_missing",
                "D3D11 did not return a staging texture",
            )
        })
    }

    fn copy_intersection_from_output(
        output_bounds: PixelRect,
        intersection: PixelRect,
        requested: PixelRect,
        source: *const u8,
        row_pitch: usize,
        canvas: &mut [u8],
    ) -> Result<(), PlatformError> {
        if source.is_null() {
            return Err(PlatformError::new(
                "dxgi_map_empty",
                "DXGI returned an empty mapped desktop surface",
            ));
        }

        let source_x = (intersection.x - output_bounds.x) as usize;
        let source_y = (intersection.y - output_bounds.y) as usize;
        let dest_x = (intersection.x - requested.x) as usize;
        let dest_y = (intersection.y - requested.y) as usize;
        let copy_width = intersection.width as usize * 4;
        let copy_height = intersection.height as usize;
        let dest_stride = requested.width as usize * 4;

        for row in 0..copy_height {
            let source_offset = (source_y + row) * row_pitch + source_x * 4;
            let dest_offset = (dest_y + row) * dest_stride + dest_x * 4;
            // SAFETY: the mapped texture is at least row_pitch bytes per row,
            // and the destination slice is the normalized requested canvas.
            let source_row =
                unsafe { slice::from_raw_parts(source.add(source_offset), copy_width) };
            canvas[dest_offset..dest_offset + copy_width].copy_from_slice(source_row);
        }

        Ok(())
    }

    fn normalize_region(region: Rect) -> Result<PixelRect, PlatformError> {
        let width = region.size.width.round() as i32;
        let height = region.size.height.round() as i32;
        if width < 1 || height < 1 {
            return Err(PlatformError::new(
                "empty_capture_region",
                "selected capture region is empty",
            ));
        }

        Ok(PixelRect {
            x: region.origin.x.round() as i32,
            y: region.origin.y.round() as i32,
            width,
            height,
        })
    }

    fn intersect(a: PixelRect, b: PixelRect) -> Option<PixelRect> {
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        let x2 = a.max_x().min(b.max_x());
        let y2 = a.max_y().min(b.max_y());

        (x2 > x1 && y2 > y1).then_some(PixelRect {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        })
    }

    fn platform_error(code: &'static str, error: WindowsError) -> PlatformError {
        PlatformError::new(code, error.message())
    }

    fn looks_like_empty_frame(bytes: &[u8]) -> bool {
        let sample_count = bytes.chunks_exact(4).take(4096).count();
        sample_count > 0
            && bytes
                .chunks_exact(4)
                .take(4096)
                .all(|pixel| pixel[0] < 2 && pixel[1] < 2 && pixel[2] < 2)
    }

    #[derive(Debug, Clone)]
    struct DxgiOutput {
        adapter: IDXGIAdapter1,
        output: IDXGIOutput1,
        bounds: PixelRect,
    }

    #[derive(Debug, Clone, Copy)]
    struct PixelRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    impl PixelRect {
        fn max_x(self) -> i32 {
            self.x + self.width
        }

        fn max_y(self) -> i32 {
            self.y + self.height
        }
    }

    struct AcquiredFrame<'a> {
        duplication: &'a IDXGIOutputDuplication,
        resource: Option<IDXGIResource>,
    }

    impl<'a> AcquiredFrame<'a> {
        fn acquire(duplication: &'a IDXGIOutputDuplication) -> Result<Self, PlatformError> {
            let mut timeout_seen = false;

            for _ in 0..FRAME_ATTEMPTS {
                let mut frame_info: DXGI_OUTDUPL_FRAME_INFO = unsafe { zeroed() };
                let mut resource = None;
                let result = unsafe {
                    duplication.AcquireNextFrame(FRAME_TIMEOUT_MS, &mut frame_info, &mut resource)
                };

                match result {
                    Ok(()) => {
                        return Ok(Self {
                            duplication,
                            resource,
                        });
                    }
                    Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                        timeout_seen = true;
                    }
                    Err(error) => return Err(platform_error("dxgi_frame_acquire_failed", error)),
                }
            }

            Err(PlatformError::new(
                "dxgi_frame_timeout",
                if timeout_seen {
                    "DXGI timed out while waiting for a desktop frame"
                } else {
                    "DXGI did not return a desktop frame"
                },
            ))
        }
    }

    impl Drop for AcquiredFrame<'_> {
        fn drop(&mut self) {
            unsafe {
                let _ = self.duplication.ReleaseFrame();
            }
        }
    }

    struct MappedTexture<'a> {
        context: &'a ID3D11DeviceContext,
        resource: &'a ID3D11Resource,
        data: *const u8,
        row_pitch: usize,
    }

    impl<'a> MappedTexture<'a> {
        fn map(
            context: &'a ID3D11DeviceContext,
            resource: &'a ID3D11Resource,
        ) -> Result<Self, PlatformError> {
            let mut mapped: D3D11_MAPPED_SUBRESOURCE = unsafe { zeroed() };
            unsafe {
                context
                    .Map(resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(|error| platform_error("dxgi_map_failed", error))?;
            }

            Ok(Self {
                context,
                resource,
                data: mapped.pData.cast::<u8>(),
                row_pitch: mapped.RowPitch as usize,
            })
        }
    }

    impl Drop for MappedTexture<'_> {
        fn drop(&mut self) {
            unsafe {
                self.context.Unmap(self.resource, 0);
            }
        }
    }
}
