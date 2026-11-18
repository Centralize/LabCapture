//! Fast screen grabbing for video on Windows, using Desktop Duplication: the
//! graphics card hands over the finished desktop picture, so even very large
//! screens can be recorded at 30-60 frames per second. Only the part being
//! recorded is copied back from the graphics card.

use image::RgbaImage;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIAdapter,
    IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
};
use windows::core::Interface;

pub struct Duplicator {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    output: IDXGIOutput1,
    duplication: Option<IDXGIOutputDuplication>,
    staging: Option<ID3D11Texture2D>,
    /// The part of the screen to copy, in the screen's pixels.
    region: (u32, u32, u32, u32),
    last: Option<RgbaImage>,
}

fn err(e: windows::core::Error) -> String {
    format!("{e}")
}

impl Duplicator {
    /// Set up duplication of the screen whose top-left corner is at
    /// (`screen_x`, `screen_y`). `region` is (x, y, w, h) inside that screen,
    /// or the whole screen when `None`.
    pub fn new(screen_x: i32, screen_y: i32, region: Option<(u32, u32, u32, u32)>) -> Result<Duplicator, String> {
        // SAFETY: plain COM calls; every object is kept alive by its smart pointer.
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(err)?;
            let mut adapter_index = 0;
            while let Ok(adapter) = factory.EnumAdapters1(adapter_index) {
                adapter_index += 1;
                let mut output_index = 0;
                while let Ok(output) = adapter.EnumOutputs(output_index) {
                    output_index += 1;
                    let desc = output.GetDesc().map_err(err)?;
                    let r = desc.DesktopCoordinates;
                    if r.left != screen_x || r.top != screen_y || !desc.AttachedToDesktop.as_bool() {
                        continue;
                    }
                    if desc.Rotation != DXGI_MODE_ROTATION_IDENTITY && desc.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED {
                        return Err("rotated screens use the standard capture".into());
                    }
                    let (sw, sh) = ((r.right - r.left) as u32, (r.bottom - r.top) as u32);
                    let region = match region {
                        Some((x, y, w, h)) => {
                            let x = x.min(sw.saturating_sub(1));
                            let y = y.min(sh.saturating_sub(1));
                            (x, y, w.min(sw - x).max(1), h.min(sh - y).max(1))
                        }
                        None => (0, 0, sw, sh),
                    };
                    return Self::open(&adapter, output, region);
                }
            }
            Err("no matching screen for fast capture".into())
        }
    }

    unsafe fn open(adapter: &IDXGIAdapter1, output: IDXGIOutput, region: (u32, u32, u32, u32)) -> Result<Duplicator, String> {
        unsafe {
            let adapter: IDXGIAdapter = adapter.cast().map_err(err)?;
            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(err)?;
            let device = device.ok_or("no graphics device")?;
            let context = context.ok_or("no graphics context")?;
            let output: IDXGIOutput1 = output.cast().map_err(err)?;
            let mut dup = Duplicator {
                device,
                context,
                output,
                duplication: None,
                staging: None,
                region,
                last: None,
            };
            dup.reconnect()?;
            Ok(dup)
        }
    }

    fn reconnect(&mut self) -> Result<(), String> {
        self.duplication = None;
        // SAFETY: COM call on live objects.
        let dup = unsafe { self.output.DuplicateOutput(&self.device) }.map_err(err)?;
        self.duplication = Some(dup);
        Ok(())
    }

    /// Start from a picture taken another way. Desktop Duplication only sends
    /// a real picture once something on screen changes, so a still screen
    /// would otherwise begin as black.
    pub fn seed(&mut self, img: RgbaImage) {
        let (_, _, w, h) = self.region;
        if img.dimensions() == (w, h) {
            self.last = Some(img);
        }
    }

    /// The newest picture of the region. Waits up to `wait_ms` for the screen
    /// to change; if nothing changed, the previous picture is returned again.
    pub fn frame(&mut self, wait_ms: u32) -> Result<RgbaImage, String> {
        for _attempt in 0..3 {
            if self.duplication.is_none() {
                self.reconnect()?;
            }
            let dup = self.duplication.clone().unwrap();
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;
            // The first picture must be waited for; later ones may be skipped.
            let wait = if self.last.is_none() { 500 } else { wait_ms };
            // SAFETY: COM call with valid out-pointers.
            let acquired = unsafe { dup.AcquireNextFrame(wait, &mut info, &mut resource) };
            match acquired {
                Ok(()) => {
                    let result = if info.LastPresentTime != 0 || self.last.is_none() {
                        match resource {
                            Some(res) => self.copy_out(&res).map(Some),
                            None => Ok(None),
                        }
                    } else {
                        Ok(None) // only the mouse moved
                    };
                    // SAFETY: we hold a frame from the call above.
                    unsafe {
                        let _ = dup.ReleaseFrame();
                    }
                    if let Some(img) = result? {
                        self.last = Some(img);
                    }
                    if let Some(img) = &self.last {
                        return Ok(img.clone());
                    }
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    if let Some(img) = &self.last {
                        return Ok(img.clone());
                    }
                }
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    // Resolution change, a full-screen game, the lock screen...
                    self.duplication = None;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => return Err(err(e)),
            }
        }
        self.last.clone().ok_or_else(|| "the screen didn't send a picture".to_string())
    }

    fn copy_out(&mut self, res: &IDXGIResource) -> Result<RgbaImage, String> {
        let (x, y, w, h) = self.region;
        // SAFETY: COM calls on live objects; the mapped memory is read inside
        // the bounds Direct3D reports and unmapped before returning.
        unsafe {
            let texture: ID3D11Texture2D = res.cast().map_err(err)?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut desc);
            let w = w.min(desc.Width.saturating_sub(x)).max(1);
            let h = h.min(desc.Height.saturating_sub(y)).max(1);
            let staging_fits = self.staging.as_ref().is_some_and(|s| {
                let mut d = D3D11_TEXTURE2D_DESC::default();
                s.GetDesc(&mut d);
                d.Width == w && d.Height == h && d.Format == desc.Format
            });
            if !staging_fits {
                let mut sd = desc;
                sd.Width = w;
                sd.Height = h;
                sd.MipLevels = 1;
                sd.ArraySize = 1;
                sd.BindFlags = 0;
                sd.MiscFlags = 0;
                sd.Usage = D3D11_USAGE_STAGING;
                sd.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                let mut staging = None;
                self.device.CreateTexture2D(&sd, None, Some(&mut staging)).map_err(err)?;
                self.staging = staging;
            }
            let staging = self.staging.as_ref().ok_or("no staging texture")?;
            let area = D3D11_BOX { left: x, top: y, right: x + w, bottom: y + h, front: 0, back: 1 };
            let dst: ID3D11Resource = staging.cast().map_err(err)?;
            let src: ID3D11Resource = texture.cast().map_err(err)?;
            self.context.CopySubresourceRegion(Some(&dst), 0, 0, 0, 0, Some(&src), 0, Some(&area));

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(Some(&dst), 0, D3D11_MAP_READ, 0, Some(&mut mapped)).map_err(err)?;
            let pitch = mapped.RowPitch as usize;
            let row_bytes = w as usize * 4;
            let src_ptr = mapped.pData as *const u8;
            let mut out = vec![0u8; row_bytes * h as usize];
            for row in 0..h as usize {
                let line = std::slice::from_raw_parts(src_ptr.add(row * pitch), row_bytes);
                let dst_line = &mut out[row * row_bytes..(row + 1) * row_bytes];
                for (d, s) in dst_line.chunks_exact_mut(4).zip(line.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = 255;
                }
            }
            self.context.Unmap(Some(&dst), 0);
            RgbaImage::from_raw(w, h, out).ok_or_else(|| "bad frame size".to_string())
        }
    }
}
