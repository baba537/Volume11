//! Application icons for the mixer rows.
//!
//! Icons are pulled straight out of each executable's resources with
//! `ExtractIconExW` and uploaded to egui once. Extraction is lazy and the result
//! — including "this executable has no icon" — is cached by path, so the work
//! happens once per program rather than once per frame.

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureOptions};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, ReleaseDC,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::{HSTRING, PCWSTR};

/// Caches one texture per executable path.
#[derive(Default)]
pub struct IconCache {
    /// `None` marks an executable whose icon could not be read, so it is not
    /// retried on every frame.
    entries: HashMap<String, Option<TextureHandle>>,
}

impl IconCache {
    /// Texture for `executable`, loading it on first use.
    pub fn get(&mut self, ctx: &egui::Context, executable: &str) -> Option<&TextureHandle> {
        if !self.entries.contains_key(executable) {
            let texture = load_image(executable).map(|image| {
                ctx.load_texture(
                    format!("appicon:{executable}"),
                    image,
                    // The icon is drawn smaller than it is stored, so linear
                    // filtering avoids the harsh edges nearest would give.
                    TextureOptions::LINEAR,
                )
            });

            self.entries.insert(executable.to_string(), texture);
        }

        self.entries.get(executable)?.as_ref()
    }

    /// Drop textures for executables that are no longer playing.
    pub fn retain(&mut self, live: &[String]) {
        self.entries
            .retain(|path, _| live.iter().any(|p| p == path));
    }
}

/// Volume11's own icon, taken from this executable's resources.
///
/// Used for the window and the taskbar. Going through the embedded resource
/// rather than a separate image file means there is exactly one icon asset and
/// no way for the two to drift apart.
pub fn own_icon(size: i32) -> Option<ColorImage> {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW};

    unsafe {
        let instance = GetModuleHandleW(None).ok()?;

        // Resource id 1 is what the build script embeds the .ico as. LoadImageW
        // picks the frame matching the requested size, which LoadIconW cannot do.
        let handle = LoadImageW(
            Some(instance.into()),
            PCWSTR(std::ptr::without_provenance(1)),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
        .ok()?;

        let icon = HICON(handle.0);
        let image = icon_to_image(icon);

        let _ = DestroyIcon(icon);

        image
    }
}

fn load_image(executable: &str) -> Option<ColorImage> {
    let icon = extract_icon(executable)?;
    let result = icon_to_image(icon);

    unsafe {
        let _ = DestroyIcon(icon);
    }

    result
}

/// Large (usually 32×32) icon of an executable.
fn extract_icon(executable: &str) -> Option<HICON> {
    let path = HSTRING::from(executable);
    let mut large = HICON::default();

    // Asking for one large icon and no small one; the large variant has enough
    // resolution for the row even on a high DPI display.
    let count = unsafe { ExtractIconExW(PCWSTR(path.as_ptr()), 0, Some(&mut large), None, 1) };

    (count > 0 && !large.is_invalid()).then_some(large)
}

fn icon_to_image(icon: HICON) -> Option<ColorImage> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;

        // Both bitmaps belong to the caller once GetIconInfo succeeds.
        let colour_bitmap = info.hbmColor;
        let mask_bitmap = info.hbmMask;

        let result = read_pixels(colour_bitmap, mask_bitmap);

        if !colour_bitmap.is_invalid() {
            let _ = DeleteObject(colour_bitmap.into());
        }
        if !mask_bitmap.is_invalid() {
            let _ = DeleteObject(mask_bitmap.into());
        }

        result
    }
}

unsafe fn read_pixels(
    colour_bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    mask_bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Option<ColorImage> {
    unsafe {
        if colour_bitmap.is_invalid() {
            return None;
        }

        let mut bitmap = BITMAP::default();
        let written = GetObjectW(
            colour_bitmap.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut _ as *mut _),
        );

        if written == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
            return None;
        }

        let width = bitmap.bmWidth as usize;
        let height = bitmap.bmHeight as usize;

        let mut pixels = read_dib(colour_bitmap, width, height)?;

        // Icons stored without an alpha channel come back fully transparent.
        // In that case the 1bpp mask decides: set bits are transparent.
        if pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0) {
            apply_mask(&mut pixels, mask_bitmap, width, height);
        }

        // GetDIBits hands back BGRA; egui wants RGBA.
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }

        Some(ColorImage::from_rgba_unmultiplied([width, height], &pixels))
    }
}

/// Read a bitmap as top-down 32bpp BGRA.
unsafe fn read_dib(
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    unsafe {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                // Negative height requests top-down rows, matching egui's order.
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut pixels = vec![0u8; width * height * 4];

        let screen = GetDC(None);
        let copied = GetDIBits(
            screen,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, screen);

        (copied != 0).then_some(pixels)
    }
}

/// Derive alpha from the icon's AND mask for icons without their own alpha.
unsafe fn apply_mask(
    pixels: &mut [u8],
    mask_bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    width: usize,
    height: usize,
) {
    unsafe {
        let Some(mask) = read_dib(mask_bitmap, width, height) else {
            // No usable mask: better a fully opaque icon than an invisible one.
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
            return;
        };

        for (pixel, mask_pixel) in pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(mask.as_chunks::<4>().0)
        {
            // The monochrome mask expands to black (draw) or white (transparent).
            let transparent = mask_pixel[0] > 127;
            pixel[3] = if transparent { 0 } else { 255 };
        }
    }
}
