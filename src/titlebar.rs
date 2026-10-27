//! Custom title bar with native window behaviour (Windows).
//!
//! The window keeps its native frame (shadow, resize borders, snapping) but the caption
//! area is drawn by egui. We replace the window procedure to:
//! - remove the native caption (`WM_NCCALCSIZE`), keeping the side and bottom borders;
//! - tell Windows which parts of our bar are caption, maximize button or top resize edge
//!   (`WM_NCHITTEST`), so dragging, double-click, Aero snap and the Windows 11
//!   snap-layouts flyout on the maximize button keep working.
//!
//! The UI publishes the title bar layout every frame with [`publish`]; the window
//! procedure reads it. On other platforms everything here is a no-op.

use eframe::egui;

/// A rect in physical client pixels.
#[derive(Clone, Copy)]
struct PxRect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl PxRect {
    const ZERO: PxRect = PxRect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 };

    fn from_egui(r: egui::Rect, pixels_per_point: f32) -> Self {
        PxRect {
            left: r.left() * pixels_per_point,
            top: r.top() * pixels_per_point,
            right: r.right() * pixels_per_point,
            bottom: r.bottom() * pixels_per_point,
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Title bar layout as last drawn by the UI.
struct BarLayout {
    bar: PxRect,
    max_button: PxRect,
    /// Widgets inside the bar (menus, search box, buttons): clicks go to egui, not to the caption.
    interactive: Vec<PxRect>,
}

static LAYOUT: std::sync::Mutex<BarLayout> =
    std::sync::Mutex::new(BarLayout { bar: PxRect::ZERO, max_button: PxRect::ZERO, interactive: Vec::new() });

/// Publishes where the bar, the maximize button and the interactive widgets are.
pub fn publish(ctx: &egui::Context, bar: egui::Rect, max_button: egui::Rect, interactive: &[egui::Rect]) {
    let ppp = ctx.pixels_per_point();
    if let Ok(mut layout) = LAYOUT.lock() {
        layout.bar = PxRect::from_egui(bar, ppp);
        layout.max_button = PxRect::from_egui(max_button, ppp);
        layout.interactive.clear();
        layout.interactive.extend(interactive.iter().map(|r| PxRect::from_egui(*r, ppp)));
    }
}

#[cfg(windows)]
pub use win::{install, max_button_state};

#[cfg(not(windows))]
pub fn install(_cc: &eframe::CreationContext<'_>) -> bool {
    false
}

#[cfg(not(windows))]
pub fn max_button_state() -> (bool, bool) {
    (false, false)
}

#[cfg(windows)]
mod win {
    use super::LAYOUT;
    use eframe::egui;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering::Relaxed};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static OLD_PROC: AtomicIsize = AtomicIsize::new(0);
    static CTX: OnceLock<egui::Context> = OnceLock::new();
    static MAX_HOVER: AtomicBool = AtomicBool::new(false);
    static MAX_PRESSED: AtomicBool = AtomicBool::new(false);

    /// Hover / pressed state of the maximize button. Windows owns its mouse input
    /// (it is reported as `HTMAXBUTTON`), so egui never sees the pointer there.
    pub fn max_button_state() -> (bool, bool) {
        (MAX_HOVER.load(Relaxed), MAX_PRESSED.load(Relaxed))
    }

    /// Replaces the native caption by the egui one. Returns false if the window
    /// handle is unavailable, in which case the native title bar stays.
    pub fn install(cc: &eframe::CreationContext<'_>) -> bool {
        let Ok(handle) = cc.window_handle() else { return false };
        let RawWindowHandle::Win32(h) = handle.as_raw() else { return false };
        let hwnd = h.hwnd.get() as HWND;
        let _ = CTX.set(cc.egui_ctx.clone());
        unsafe {
            let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wndproc as *const () as isize);
            OLD_PROC.store(old, Relaxed);
            // Re-run WM_NCCALCSIZE so the caption disappears right away.
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        true
    }

    fn repaint() {
        if let Some(ctx) = CTX.get() {
            ctx.request_repaint();
        }
    }

    unsafe fn call_old(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let old: WNDPROC = std::mem::transmute(OLD_PROC.load(Relaxed));
            CallWindowProcW(old, hwnd, msg, wparam, lparam)
        }
    }

    /// Resize border thickness (x, y) in physical pixels.
    unsafe fn frame_thickness(hwnd: HWND) -> (i32, i32) {
        unsafe {
            let dpi = GetDpiForWindow(hwnd);
            let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
            (GetSystemMetricsForDpi(SM_CXFRAME, dpi) + pad, GetSystemMetricsForDpi(SM_CYFRAME, dpi) + pad)
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let on_max = wparam == HTMAXBUTTON as WPARAM;
            match msg {
                WM_NCCALCSIZE if wparam != 0 => {
                    // Keep the native left/right/bottom borders, drop the caption.
                    let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                    let r = &mut params.rgrc[0];
                    let (fx, fy) = frame_thickness(hwnd);
                    r.left += fx;
                    r.right -= fx;
                    r.bottom -= fy;
                    if IsZoomed(hwnd) != 0 {
                        // A maximized window overhangs the screen by the frame size.
                        r.top += fy;
                    }
                    return 0;
                }
                WM_NCHITTEST => {
                    let hit = call_old(hwnd, msg, wparam, lparam);
                    if hit != HTCLIENT as LRESULT {
                        return hit;
                    }
                    return hit_test(hwnd, lparam);
                }
                WM_NCMOUSEMOVE => {
                    if MAX_HOVER.swap(on_max, Relaxed) != on_max {
                        repaint();
                    }
                    if on_max {
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE | TME_NONCLIENT,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        TrackMouseEvent(&mut tme);
                        return 0;
                    }
                }
                WM_NCMOUSELEAVE => {
                    MAX_HOVER.store(false, Relaxed);
                    MAX_PRESSED.store(false, Relaxed);
                    repaint();
                }
                WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if on_max => {
                    MAX_PRESSED.store(true, Relaxed);
                    repaint();
                    return 0;
                }
                WM_NCLBUTTONUP if on_max => {
                    if MAX_PRESSED.swap(false, Relaxed) {
                        ShowWindow(hwnd, if IsZoomed(hwnd) != 0 { SW_RESTORE } else { SW_MAXIMIZE });
                    }
                    repaint();
                    return 0;
                }
                _ => {}
            }
            call_old(hwnd, msg, wparam, lparam)
        }
    }

    unsafe fn hit_test(hwnd: HWND, lparam: LPARAM) -> LRESULT {
        unsafe {
            let mut pt = POINT { x: (lparam & 0xffff) as i16 as i32, y: ((lparam >> 16) & 0xffff) as i16 as i32 };
            ScreenToClient(hwnd, &mut pt);
            let (x, y) = (pt.x as f32, pt.y as f32);

            // The caption is gone, so the top resize edge lives inside the client area.
            let (fx, fy) = frame_thickness(hwnd);
            if IsZoomed(hwnd) == 0 && pt.y < fy {
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                return if pt.x < fx * 2 {
                    HTTOPLEFT
                } else if pt.x >= rc.right - fx * 2 {
                    HTTOPRIGHT
                } else {
                    HTTOP
                } as LRESULT;
            }

            let Ok(layout) = LAYOUT.try_lock() else { return HTCLIENT as LRESULT };
            if layout.max_button.contains(x, y) {
                return HTMAXBUTTON as LRESULT;
            }
            if layout.bar.contains(x, y) && !layout.interactive.iter().any(|r| r.contains(x, y)) {
                return HTCAPTION as LRESULT;
            }
            HTCLIENT as LRESULT
        }
    }
}
