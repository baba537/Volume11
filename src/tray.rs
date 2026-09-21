//! Tray icon, on its own thread with its own Win32 message loop.
//!
//! Deliberately not a crate: the surface needed here is small, and keeping it in
//! process avoids pulling another dependency tree in for one icon.
//!
//! Two details the old WPF version got wrong are handled here: the icon is loaded
//! from the executable's own resources rather than a file path relative to the
//! working directory, and it is explicitly removed again on shutdown so no ghost
//! icon is left behind in the notification area.

use std::sync::{Mutex, OnceLock};

use crossbeam_channel::{Receiver, Sender, unbounded};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, HICON, HMENU, IDI_APPLICATION,
    IMAGE_ICON, LR_DEFAULTCOLOR, LoadIconW, LoadImageW, MF_SEPARATOR, MF_STRING, MSG,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SM_CXSMICON, SetForegroundWindow,
    TPM_BOTTOMALIGN, TPM_RIGHTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP,
    WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};
use windows::core::{HSTRING, PCWSTR, w};

/// Custom message the shell sends us for mouse activity on the icon.
const WM_TRAY: u32 = WM_APP + 1;

/// Broadcast by a second start of Volume11 asking this one to show itself.
static SHOW_REQUEST: OnceLock<u32> = OnceLock::new();

/// Broadcast by the installer asking this instance to shut down.
static QUIT_REQUEST: OnceLock<u32> = OnceLock::new();

/// `TaskbarCreated`, broadcast when Explorer restarts. Without handling it the
/// tray icon disappears for good after an Explorer crash.
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();
/// The icon data, so it can be re-added when Explorer comes back.
static ICON_DATA: Mutex<Option<IconData>> = Mutex::new(None);

/// `NOTIFYICONDATAW` is not `Send` by default because of its raw handles, but the
/// handles here are process-wide and only ever used from the tray thread's own
/// message loop or from a broadcast handled on that same thread.
struct IconData(NOTIFYICONDATAW);
unsafe impl Send for IconData {}

const MENU_OPEN: usize = 1;
const MENU_SETTINGS: usize = 2;
const MENU_EXIT: usize = 3;

/// What the tray thread reports to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMessage {
    /// Left click: show the mixer if hidden, hide it if already visible.
    Toggle,
    /// Context menu "Open".
    Show,
    /// Context menu "Settings".
    ShowSettings,
    /// Context menu "Exit".
    Quit,
}

/// Sender used from inside the window procedure, which cannot carry state.
static SENDER: OnceLock<Sender<TrayMessage>> = OnceLock::new();
/// Called after every tray message so the UI thread wakes up and reacts.
static WAKE: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

fn notify(message: TrayMessage) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(message);
    }
    if let Some(wake) = WAKE.get() {
        wake();
    }
}

/// Start the tray thread. The returned receiver stays valid for the process
/// lifetime; the thread ends when the process does.
pub fn spawn<F>(wake: F) -> Receiver<TrayMessage>
where
    F: Fn() + Send + Sync + 'static,
{
    let (tx, rx) = unbounded();

    let _ = SENDER.set(tx);
    let _ = WAKE.set(Box::new(wake));

    std::thread::Builder::new()
        .name("volume11-tray".into())
        .spawn(|| unsafe { run() })
        .expect("tray thread must start");

    rx
}

unsafe fn run() {
    unsafe {
        let instance = GetModuleHandleW(None).expect("module handle");

        let class_name = w!("Volume11TrayWindow");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&class);

        let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(w!("TaskbarCreated")));
        let _ = SHOW_REQUEST.set(RegisterWindowMessageW(crate::instance::SHOW_MESSAGE_NAME));
        let _ = QUIT_REQUEST.set(RegisterWindowMessageW(crate::instance::QUIT_MESSAGE_NAME));

        // A hidden tool window rather than HWND_MESSAGE: message-only windows do
        // not receive the `TaskbarCreated` broadcast. `WS_EX_TOOLWINDOW` keeps it
        // out of the taskbar and out of Alt+Tab, and stops it from being mistaken
        // for the application's main window.
        let window = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name,
            w!("Volume11"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .expect("tray window");

        // Resource id 1 is the application icon embedded by the build script.
        //
        // LoadIconW is deliberately not used here: it always returns the large
        // (32 px) icon, which Windows then squashes into the 16 px notification
        // area. LoadImageW picks the frame that actually matches that size, so
        // the icon stays legible. Falling back to the generic application icon
        // keeps the tray usable if the resource is ever missing.
        let size = GetSystemMetrics(SM_CXSMICON);

        let icon: HICON = LoadImageW(
            Some(instance.into()),
            PCWSTR(std::ptr::without_provenance(1)),
            IMAGE_ICON,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
        .map(|handle| HICON(handle.0))
        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
        .unwrap_or_default();

        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: window,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: icon,
            ..Default::default()
        };

        let tip: Vec<u16> = "Volume11".encode_utf16().collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);

        let _ = Shell_NotifyIconW(NIM_ADD, &data);

        if let Ok(mut stored) = ICON_DATA.lock() {
            *stored = Some(IconData(data));
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        // Without this the icon lingers until the user hovers over it.
        if let Ok(stored) = ICON_DATA.lock()
            && let Some(icon_data) = stored.as_ref()
        {
            let _ = Shell_NotifyIconW(NIM_DELETE, &icon_data.0);
        }
        let _ = DestroyWindow(window);
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        // Explorer restarted: every tray icon has to be registered again.
        if let Some(&taskbar_created) = TASKBAR_CREATED.get()
            && message == taskbar_created
            && taskbar_created != 0
            && let Ok(stored) = ICON_DATA.lock()
            && let Some(icon_data) = stored.as_ref()
        {
            let _ = Shell_NotifyIconW(NIM_ADD, &icon_data.0);
            return LRESULT(0);
        }

        // The installer is about to replace or remove the executable.
        if let Some(&quit_request) = QUIT_REQUEST.get()
            && message == quit_request
            && quit_request != 0
        {
            notify(TrayMessage::Quit);
            return LRESULT(0);
        }

        // A second copy was started; bring this one forward instead.
        if let Some(&show_request) = SHOW_REQUEST.get()
            && message == show_request
            && show_request != 0
        {
            notify(TrayMessage::Show);
            return LRESULT(0);
        }

        match message {
            WM_TRAY => {
                match lparam.0 as u32 {
                    WM_LBUTTONUP => notify(TrayMessage::Toggle),
                    WM_RBUTTONUP => show_menu(window),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                match wparam.0 & 0xFFFF {
                    MENU_OPEN => notify(TrayMessage::Show),
                    MENU_SETTINGS => notify(TrayMessage::ShowSettings),
                    MENU_EXIT => notify(TrayMessage::Quit),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

unsafe fn show_menu(window: HWND) {
    unsafe {
        let Ok(menu): windows::core::Result<HMENU> = CreatePopupMenu() else {
            return;
        };

        let _ = AppendMenuW(menu, MF_STRING, MENU_OPEN, &HSTRING::from("Open"));
        let _ = AppendMenuW(menu, MF_STRING, MENU_SETTINGS, &HSTRING::from("Settings"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT, &HSTRING::from("Quit"));

        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);

        // Required so the menu closes when the user clicks elsewhere.
        let _ = SetForegroundWindow(window);

        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            None,
            window,
            None,
        );

        let _ = DestroyMenu(menu);
    }
}
