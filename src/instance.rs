//! Single instance.
//!
//! Two copies of Volume11 fight each other: both put an icon in the tray and
//! both write the configuration file. A named mutex claims the slot; a second
//! start tells the running copy to show itself and exits.

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, LPARAM, WPARAM,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, PostMessageW, RegisterWindowMessageW,
};
use windows::core::w;

/// Named per user, not per machine: two people on the same PC each get their own.
const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\Volume11.SingleInstance");

/// Broadcast by a second start, handled by the running instance.
pub const SHOW_MESSAGE_NAME: windows::core::PCWSTR = w!("Volume11.ShowWindow");

/// Broadcast by the installer before it replaces or removes the executable.
///
/// A running Volume11 holds its own file open, so an update would fail. The
/// installer asks it to quit first, which also gives it the chance to save its
/// configuration. This is a separate message from the show request so nothing
/// else can shut the application down by accident.
pub const QUIT_MESSAGE_NAME: windows::core::PCWSTR = w!("Volume11.QuitNow");

/// Holds the mutex for as long as this process runs.
pub struct Guard(HANDLE);

impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Claim the single-instance slot.
///
/// `None` means another copy already has it.
pub fn acquire() -> Option<Guard> {
    unsafe {
        let handle = CreateMutexW(None, true, MUTEX_NAME).ok()?;

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }

        Some(Guard(handle))
    }
}

/// Ask the running instance to open its window.
///
/// Broadcast rather than aimed at a handle, because finding the other process's
/// window would mean enumerating and matching on a class name. The message id is
/// registered from a string, so only Volume11 reacts to it.
pub fn signal_existing() {
    unsafe {
        let message = RegisterWindowMessageW(SHOW_MESSAGE_NAME);

        if message != 0 {
            let _ = PostMessageW(Some(HWND_BROADCAST), message, WPARAM(0), LPARAM(0));
        }
    }
}
