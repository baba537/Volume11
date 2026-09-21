//! Where the mixer window opens.
//!
//! Always the bottom right corner of the primary monitor, above the taskbar,
//! whatever its resolution or scaling. The window belongs next to the tray icon
//! that opens it, and that icon lives on the primary monitor.
//!
//! Two Windows quirks are handled here. `rcWork` already excludes the taskbar,
//! but only when the taskbar reserves space — set to hide automatically it
//! reserves none, so its rectangle is subtracted separately. And the monitor the
//! window is currently on may scale differently from the primary one, so the
//! conversion between physical pixels and the points egui expects is done with
//! both factors rather than one.

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    ABE_BOTTOM, ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETTASKBARPOS, APPBARDATA, SHAppBarMessage,
};

/// Work area of the primary monitor in physical pixels, plus its scale factor.
///
/// The primary monitor is found by asking which monitor contains the virtual
/// desktop origin: Windows defines (0, 0) to be its top left corner regardless
/// of how the other screens are arranged around it.
fn primary_work_area() -> Option<(RECT, f32)> {
    unsafe {
        let monitor: HMONITOR = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);

        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }

        let mut work = info.rcWork;

        // An auto-hiding taskbar reserves no work area, so the window would sit
        // exactly where the taskbar slides back in.
        if let Some((edge, bar)) = taskbar()
            && bar.right > info.rcMonitor.left
            && bar.left < info.rcMonitor.right
            && bar.bottom > info.rcMonitor.top
            && bar.top < info.rcMonitor.bottom
        {
            match edge {
                ABE_LEFT => work.left = work.left.max(bar.right),
                ABE_TOP => work.top = work.top.max(bar.bottom),
                ABE_RIGHT => work.right = work.right.min(bar.left),
                ABE_BOTTOM => work.bottom = work.bottom.min(bar.top),
                _ => {}
            }
        }

        let mut dpi_x = 96;
        let mut dpi_y = 96;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

        let scale = if dpi_x == 0 { 1.0 } else { dpi_x as f32 / 96.0 };

        Some((work, scale))
    }
}

/// Edge and rectangle of the taskbar, if Windows reports one.
fn taskbar() -> Option<(u32, RECT)> {
    unsafe {
        let mut data = APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            ..Default::default()
        };

        if SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) == 0 {
            return None;
        }

        Some((data.uEdge, data.rc))
    }
}

/// Bottom right corner of the primary monitor, in the points egui expects.
///
/// `size` is the window in logical points and `current_scale` the factor of the
/// monitor the window happens to be on right now. The window is laid out at the
/// primary monitor's scale once it arrives there, so its physical size is
/// computed with that factor, while the final coordinate is divided by the
/// current one because that is what `ViewportCommand::OuterPosition` multiplies
/// by. On a single-monitor machine the two are identical and this reduces to the
/// obvious arithmetic.
pub fn bottom_right(size: egui::Vec2, current_scale: f32) -> Option<egui::Pos2> {
    let (work, primary_scale) = primary_work_area()?;

    let current_scale = if current_scale > 0.0 {
        current_scale
    } else {
        1.0
    };

    let margin = 12.0 * primary_scale;
    let width = size.x * primary_scale;
    let height = size.y * primary_scale;

    let x = (work.right as f32 - width - margin).max(work.left as f32);
    let y = (work.bottom as f32 - height - margin).max(work.top as f32);

    Some(egui::pos2(x / current_scale, y / current_scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_monitor_is_answerable() {
        let (work, scale) = primary_work_area().expect("a primary monitor must exist");

        assert!(work.right > work.left, "work area must have width");
        assert!(work.bottom > work.top, "work area must have height");
        assert!(scale > 0.0, "scale must be positive");
    }

    #[test]
    fn window_lands_inside_the_work_area() {
        let (work, scale) = primary_work_area().expect("a primary monitor must exist");
        let size = egui::vec2(372.0, 500.0);

        let position = bottom_right(size, scale).expect("a position must be produced");

        // Converted back to physical pixels, the whole window must fit.
        let left = position.x * scale;
        let top = position.y * scale;

        assert!(
            left >= work.left as f32,
            "left edge {left} outside work area"
        );
        assert!(top >= work.top as f32, "top edge {top} outside work area");
        assert!(
            left + size.x * scale <= work.right as f32,
            "right edge outside work area"
        );
        assert!(
            top + size.y * scale <= work.bottom as f32,
            "bottom edge outside work area"
        );
    }
}
