//! "Start with Windows", via the per-user `Run` key.
//!
//! HKCU only, so it never needs elevation.
//!
//! Two registry locations are involved, because Task Manager's Startup tab does
//! not delete `Run` entries when you switch one off. It writes a status byte to
//! `Explorer\StartupApproved\Run` instead and leaves the command in place. To let
//! the checkbox here and the Task Manager switch mean the same thing, Volume11
//! writes both: the command under `Run`, and the status byte next to it.
//!
//! The consequence is deliberate: turning autostart off leaves a disabled entry
//! listed in Task Manager rather than removing it, which is exactly how every
//! other application behaves there.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, FILETIME};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_BINARY, REG_OPTION_NON_VOLATILE,
    REG_SAM_FLAGS, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows::core::{HSTRING, PCWSTR, w};

const RUN_KEY: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED_KEY: PCWSTR =
    w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
const VALUE_NAME: PCWSTR = w!("Volume11");

/// Status byte layout used by Task Manager. Bit 0 set means "disabled"; the
/// remaining bytes hold the time the entry was switched off.
const STATUS_LENGTH: usize = 12;
const STATUS_ENABLED: u8 = 0x02;
const STATUS_DISABLED: u8 = 0x03;

fn open(key: PCWSTR, access: REG_SAM_FLAGS) -> Option<HKEY> {
    let mut handle = HKEY::default();
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, key, None, access, &mut handle) };
    status.is_ok().then_some(handle)
}

/// Open for writing, creating the key when it does not exist yet.
/// `StartupApproved\Run` is absent until something has been toggled at least once.
fn create(key: PCWSTR) -> Option<HKEY> {
    let mut handle = HKEY::default();

    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            key,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_READ,
            None,
            &mut handle,
            None,
        )
    };

    status.is_ok().then_some(handle)
}

fn close(key: HKEY) {
    unsafe {
        let _ = RegCloseKey(key);
    }
}

/// Write the quoted path of this executable to the `Run` key.
fn write_run_command() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| format!("Path: {error}"))?;

    // Quoted, because the path can contain spaces.
    let command = HSTRING::from(format!("\"{}\"", executable.display()));

    // Length in bytes, including the terminating NUL.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            command.as_ptr() as *const u8,
            (command.len() + 1) * std::mem::size_of::<u16>(),
        )
    };

    let key = create(RUN_KEY).ok_or("Registry locked")?;

    let result = unsafe { RegSetValueExW(key, VALUE_NAME, None, REG_SZ, Some(bytes)) }
        .ok()
        .map_err(|error| error.to_string());

    close(key);
    result
}

/// Write the Task Manager status byte.
fn write_status(enabled: bool) -> Result<(), String> {
    let key = create(APPROVED_KEY).ok_or("Registry locked")?;

    let mut status = [0u8; STATUS_LENGTH];
    status[0] = if enabled {
        STATUS_ENABLED
    } else {
        STATUS_DISABLED
    };

    if !enabled {
        // Task Manager stores when the entry was switched off in bytes 4..12.
        let now: FILETIME = unsafe { GetSystemTimeAsFileTime() };

        status[4..8].copy_from_slice(&now.dwLowDateTime.to_le_bytes());
        status[8..12].copy_from_slice(&now.dwHighDateTime.to_le_bytes());
    }

    let result = unsafe { RegSetValueExW(key, VALUE_NAME, None, REG_BINARY, Some(&status)) }
        .ok()
        .map_err(|error| error.to_string());

    close(key);
    result
}

/// Turn autostart on or off.
///
/// Switching off keeps the `Run` command and marks it disabled, so the entry
/// stays listed in Task Manager and can be switched back on from there.
pub fn set(enabled: bool) -> Result<(), String> {
    write_run_command()?;
    write_status(enabled)
}

/// Remove every trace of the autostart entry.
///
/// Not wired to the checkbox, which only disables. Kept for an uninstall path
/// and so the registry can be cleaned up deliberately.
pub fn remove() -> Result<(), String> {
    for key_path in [RUN_KEY, APPROVED_KEY] {
        let Some(key) = open(key_path, KEY_WRITE) else {
            continue;
        };

        let status = unsafe { RegDeleteValueW(key, VALUE_NAME) };
        close(key);

        // Deleting something that is not there is the desired end state.
        if status.is_err() && status != ERROR_FILE_NOT_FOUND {
            return Err(format!("{status:?}"));
        }
    }

    Ok(())
}

/// The command currently stored under `Run`, if any.
fn run_command() -> Option<String> {
    let key = open(RUN_KEY, KEY_READ)?;

    // Ask for the size first; a path can be longer than any fixed buffer.
    let mut length: u32 = 0;
    let sized = unsafe { RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut length)) };

    if sized.is_err() || length == 0 {
        close(key);
        return None;
    }

    let mut buffer = vec![0u16; (length as usize).div_ceil(2)];
    let read = unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            None,
            Some(buffer.as_mut_ptr() as *mut u8),
            Some(&mut length),
        )
    };
    close(key);

    if read.is_err() {
        return None;
    }

    let text = String::from_utf16_lossy(&buffer);
    Some(text.trim_end_matches('\0').to_string())
}

/// Path of the executable a `Run` command starts, without quotes or arguments.
fn command_target(command: &str) -> &str {
    let command = command.trim();

    if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest)
    } else {
        command.split(' ').next().unwrap_or(command)
    }
}

/// Point a dead autostart entry at this executable.
///
/// The entry is written with the path of whichever copy switched it on. Once
/// that copy is deleted — a download replaced by the installed version, a
/// portable folder removed — Windows starts a file that is no longer there,
/// while the settings panel still reports autostart as on.
///
/// Only a *missing* target is replaced. Repointing whenever the running copy
/// differs would let any copy that is merely started once — a portable one on a
/// stick, a build being tested — take over a working entry. An old copy that
/// still exists next to a new install is handled by the installer, which
/// adopts the entry when it runs. The Task Manager status byte is left alone,
/// so a disabled entry stays disabled.
///
/// Returns whether the command was changed.
pub fn repair() -> bool {
    let Some(command) = run_command() else {
        return false;
    };

    let target = command_target(&command);
    if !needs_repair(target, std::path::Path::new(target).exists()) {
        return false;
    }

    write_run_command().is_ok()
}

/// Whether an entry pointing at `target` should be rewritten.
fn needs_repair(target: &str, target_exists: bool) -> bool {
    !target.trim().is_empty() && !target_exists
}

/// Whether the `Run` value exists at all, regardless of its status byte.
fn run_command_exists() -> bool {
    let Some(key) = open(RUN_KEY, KEY_READ) else {
        return false;
    };

    let status = unsafe { RegQueryValueExW(key, VALUE_NAME, None, None, None, None) };
    close(key);

    status.is_ok()
}

/// Whether Volume11 will actually be started by Windows.
///
/// Reads the same two places Task Manager does, so toggling it there is picked up
/// on the next start of the settings panel.
pub fn is_enabled() -> bool {
    if !run_command_exists() {
        return false;
    }

    let Some(key) = open(APPROVED_KEY, KEY_READ) else {
        // No StartupApproved key means nothing has ever been switched off.
        return true;
    };

    let mut buffer = [0u8; STATUS_LENGTH];
    let mut length = buffer.len() as u32;

    let status = unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut length),
        )
    };

    close(key);

    if status.is_err() || length == 0 {
        // Present in Run but never toggled: Windows will start it.
        return true;
    }

    // Bit 0 marks the entry as disabled.
    buffer[0] & 1 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_byte_encodes_enabled_state() {
        // Mirrors the check in `is_enabled`, so the constants cannot drift apart
        // from the meaning documented above.
        assert_eq!(STATUS_ENABLED & 1, 0, "0x02 must read as enabled");
        assert_eq!(STATUS_DISABLED & 1, 1, "0x03 must read as disabled");
    }

    #[test]
    fn command_target_handles_quotes_and_arguments() {
        assert_eq!(
            command_target(r#""C:\Program Files\Volume11\Volume11.exe""#),
            r"C:\Program Files\Volume11\Volume11.exe"
        );
        assert_eq!(
            command_target(r#""C: b\Volume11.exe" --show"#),
            r"C: b\Volume11.exe"
        );
        assert_eq!(
            command_target(r"C:\Tools\Volume11.exe --show"),
            r"C:\Tools\Volume11.exe"
        );
    }

    #[test]
    fn only_a_missing_target_is_repaired() {
        // Dead entry, e.g. a deleted download: repoint.
        assert!(needs_repair(r"C:\Users\x\Downloads\Volume11.exe", false));
        // Working entry: a portable or test copy must not take it over.
        assert!(!needs_repair(r"C:\Programs\Volume11\Volume11.exe", true));
        // Nothing usable stored: leave it.
        assert!(!needs_repair("  ", false));
    }

    #[test]
    fn reading_the_state_does_not_panic() {
        // Whatever the machine's registry looks like, this must stay answerable.
        let _ = is_enabled();
    }
}
