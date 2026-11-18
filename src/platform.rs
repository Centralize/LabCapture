//! Small operating-system helpers: where the mouse is, and keeping our own
//! window out of captures.

/// The mouse pointer, in physical screen pixels.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub left_down: bool,
    pub right_down: bool,
}

#[cfg(windows)]
pub fn pointer() -> Option<Pointer> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CURSOR_SHOWING, CURSORINFO, GetCursorInfo};
    // SAFETY: plain Win32 queries on a zeroed, correctly sized struct.
    unsafe {
        let mut info: CURSORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<CURSORINFO>() as u32;
        if GetCursorInfo(&mut info) == 0 {
            return None;
        }
        let left = (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0;
        let right = (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0;
        Some(Pointer {
            x: info.ptScreenPos.x,
            y: info.ptScreenPos.y,
            visible: (info.flags & CURSOR_SHOWING) != 0,
            left_down: left,
            right_down: right,
        })
    }
}

#[cfg(not(windows))]
pub fn pointer() -> Option<Pointer> {
    None
}

/// Whether this system can keep a window out of screen captures.
pub fn can_exclude_from_capture() -> bool {
    cfg!(windows)
}

/// Keep (or stop keeping) a window out of screenshots and recordings.
/// `hwnd` is the native window handle. Returns true when it worked.
#[cfg(windows)]
pub fn exclude_from_capture(hwnd: isize, exclude: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    };
    if hwnd == 0 {
        return false;
    }
    let affinity = if exclude { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE };
    // SAFETY: hwnd comes from our own live window.
    unsafe { SetWindowDisplayAffinity(hwnd as _, affinity) != 0 }
}

#[cfg(not(windows))]
pub fn exclude_from_capture(_hwnd: isize, _exclude: bool) -> bool {
    false
}

/// Open the system file manager with `path` selected.
pub fn show_in_folder(path: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(parent) = path.parent() {
            let _ = open::that(parent);
        }
    }
}
