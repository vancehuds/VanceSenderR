//! Win32 floating status overlay — non-activating topmost window for send progress.
//!
//! Creates a small dark panel in the top-right corner of the primary monitor.
//! Shows send status text and auto-hides after a timeout.

#[cfg(windows)]
#[allow(unused_must_use)]
mod win32 {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontIndirectW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint,
        FillRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor, DT_END_ELLIPSIS,
        DT_LEFT, DT_SINGLELINE, DT_VCENTER, FONT_CHARSET, LOGFONTW, PAINTSTRUCT, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetSystemMetrics,
        PeekMessageW, PostMessageW, RegisterClassExW, SetWindowPos, ShowWindow, TranslateMessage,
        CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, MSG, PM_REMOVE, SM_CXSCREEN, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, WM_DESTROY,
        WM_PAINT, WM_USER, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_POPUP,
    };

    const WINDOW_WIDTH: i32 = 320;
    const WINDOW_HEIGHT: i32 = 36;
    const MARGIN_RIGHT: i32 = 16;
    const MARGIN_TOP: i32 = 16;
    const AUTO_HIDE_MS: u64 = 3000;
    const BG_COLOR: u32 = 0x003C3C3C; // Dark gray (BGR)
    const TEXT_COLOR: u32 = 0x00E0E0E0; // Light gray (BGR)

    const WM_STATUS_UPDATE: u32 = WM_USER + 100;
    const WM_STATUS_HIDE: u32 = WM_USER + 101;
    const WM_QUIT_OVERLAY: u32 = WM_USER + 102;

    struct OverlayState {
        text: String,
        last_update: Instant,
        visible: bool,
    }

    // Thread-local state accessed only from the overlay thread
    static OVERLAY_STATE: Mutex<Option<OverlayState>> = Mutex::new(None);
    // Shared text buffer for cross-thread communication
    static PENDING_TEXT: Mutex<Option<String>> = Mutex::new(None);

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

                // Draw background
                let bg_brush = unsafe {
                    CreateSolidBrush(windows::Win32::Foundation::COLORREF(BG_COLOR))
                };
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: WINDOW_WIDTH,
                    bottom: WINDOW_HEIGHT,
                };
                let _ = unsafe { FillRect(hdc, &rect, bg_brush) };

                // Draw text
                let text = {
                    let state = OVERLAY_STATE.lock().unwrap();
                    state.as_ref().map(|s| s.text.clone()).unwrap_or_default()
                };

                if !text.is_empty() {
                    unsafe {
                        let _ = SetBkMode(hdc, TRANSPARENT);
                        let _ = SetTextColor(hdc, windows::Win32::Foundation::COLORREF(TEXT_COLOR));
                    }

                    // Create a reasonable font
                    let mut lf = LOGFONTW::default();
                    lf.lfHeight = -14;
                    lf.lfWeight = 400;
                    lf.lfCharSet = FONT_CHARSET(1); // DEFAULT_CHARSET
                    let font_name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
                    let copy_len = font_name.len().min(32);
                    lf.lfFaceName[..copy_len].copy_from_slice(&font_name[..copy_len]);
                    let hfont = unsafe { CreateFontIndirectW(&lf) };
                    let old_font = unsafe { SelectObject(hdc, hfont.into()) };

                    let mut wide: Vec<u16> = text.encode_utf16().collect();
                    rect.left = 10;
                    rect.right = WINDOW_WIDTH - 10;
                    unsafe {
                        DrawTextW(
                            hdc,
                            &mut wide,
                            &mut rect,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
                        );
                    }
                    unsafe {
                        let _ = SelectObject(hdc, old_font);
                        let _ = DeleteObject(hfont.into());
                    }
                }

                unsafe {
                    let _ = DeleteObject(bg_brush.into());
                    let _ = EndPaint(hwnd, &ps);
                }
                LRESULT(0)
            }
            WM_STATUS_UPDATE => {
                // Read pending text
                let text = PENDING_TEXT.lock().unwrap().take().unwrap_or_default();
                let mut state = OVERLAY_STATE.lock().unwrap();
                if let Some(ref mut s) = *state {
                    s.text = text;
                    s.last_update = Instant::now();
                    if !s.visible {
                        unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
                        s.visible = true;
                    }
                }
                drop(state);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, true) };
                LRESULT(0)
            }
            WM_STATUS_HIDE => {
                let mut state = OVERLAY_STATE.lock().unwrap();
                if let Some(ref mut s) = *state {
                    unsafe { ShowWindow(hwnd, SW_HIDE) };
                    s.visible = false;
                    s.text.clear();
                }
                LRESULT(0)
            }
            WM_QUIT_OVERLAY => {
                let _ = unsafe { DestroyWindow(hwnd) };
                LRESULT(0)
            }
            WM_DESTROY => {
                *OVERLAY_STATE.lock().unwrap() = None;
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    /// Wrapper to make HWND Send-safe (it's just a pointer, safe across threads).
    struct SendHwnd(HWND);
    unsafe impl Send for SendHwnd {}
    unsafe impl Sync for SendHwnd {}

    /// Handle to send commands to the running overlay window.
    #[derive(Clone)]
    pub struct StatusOverlayHandle {
        hwnd: Arc<Mutex<Option<SendHwnd>>>,
        running: Arc<AtomicBool>,
    }

    impl StatusOverlayHandle {
        /// Show a status message on the overlay.
        pub fn show_status(&self, text: &str) {
            let guard = self.hwnd.lock().unwrap();
            if let Some(ref sh) = *guard {
                *PENDING_TEXT.lock().unwrap() = Some(text.to_string());
                let _ = unsafe { PostMessageW(Some(sh.0), WM_STATUS_UPDATE, WPARAM(0), LPARAM(0)) };
            }
        }

        /// Hide the overlay.
        #[allow(dead_code)]
        pub fn hide(&self) {
            let guard = self.hwnd.lock().unwrap();
            if let Some(ref sh) = *guard {
                let _ = unsafe { PostMessageW(Some(sh.0), WM_STATUS_HIDE, WPARAM(0), LPARAM(0)) };
            }
        }

        /// Destroy the overlay window and stop the thread.
        pub fn destroy(&self) {
            self.running.store(false, Ordering::SeqCst);
            let guard = self.hwnd.lock().unwrap();
            if let Some(ref sh) = *guard {
                let _ = unsafe { PostMessageW(Some(sh.0), WM_QUIT_OVERLAY, WPARAM(0), LPARAM(0)) };
            }
        }
    }

    /// Start the status overlay on a new thread and return a handle.
    pub fn start_status_overlay() -> StatusOverlayHandle {
        let running = Arc::new(AtomicBool::new(true));
        let hwnd_arc: Arc<Mutex<Option<SendHwnd>>> = Arc::new(Mutex::new(None));

        let running_clone = running.clone();
        let hwnd_clone = hwnd_arc.clone();

        thread::Builder::new()
            .name("status-overlay".into())
            .spawn(move || {
                run_overlay_thread(running_clone, hwnd_clone);
            })
            .expect("failed to spawn status overlay thread");

        StatusOverlayHandle {
            hwnd: hwnd_arc,
            running,
        }
    }

    fn run_overlay_thread(
        running: Arc<AtomicBool>,
        hwnd_out: Arc<Mutex<Option<SendHwnd>>>,
    ) {
        // Register window class
        let class_name_wide: Vec<u16> = "VanceSenderStatusOverlay\0".encode_utf16().collect();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: PCWSTR(class_name_wide.as_ptr()),
            ..Default::default()
        };

        unsafe { RegisterClassExW(&wc) };

        // Position: top-right corner
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let x = screen_w - WINDOW_WIDTH - MARGIN_RIGHT;
        let y = MARGIN_TOP;

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                PCWSTR(class_name_wide.as_ptr()),
                PCWSTR::null(),
                WS_POPUP,
                x,
                y,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                None, // parent
                None, // menu
                None, // hinstance
                None, // param
            )
        }
        .expect("failed to create status overlay window");

        // Set topmost
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        };

        // Start hidden
        unsafe { ShowWindow(hwnd, SW_HIDE) };

        // Initialize state
        *OVERLAY_STATE.lock().unwrap() = Some(OverlayState {
            text: String::new(),
            last_update: Instant::now(),
            visible: false,
        });

        // Store hwnd for external access
        *hwnd_out.lock().unwrap() = Some(SendHwnd(hwnd));

        // Message loop with auto-hide timer
        let mut msg = MSG::default();
        while running.load(Ordering::Relaxed) {
            // Process all pending messages
            while unsafe {
                PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE)
            }
            .as_bool()
            {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // Auto-hide check
            {
                let mut state = OVERLAY_STATE.lock().unwrap();
                if let Some(ref mut s) = *state {
                    if s.visible
                        && s.last_update.elapsed() > Duration::from_millis(AUTO_HIDE_MS)
                    {
                        unsafe { ShowWindow(hwnd, SW_HIDE) };
                        s.visible = false;
                        s.text.clear();
                    }
                }
            }

            thread::sleep(Duration::from_millis(50));
        }

        // Cleanup
        if OVERLAY_STATE.lock().unwrap().is_some() {
            let _ = unsafe { DestroyWindow(hwnd) };
            *OVERLAY_STATE.lock().unwrap() = None;
        }
        *hwnd_out.lock().unwrap() = None;
    }
}

#[cfg(windows)]
pub use win32::{start_status_overlay, StatusOverlayHandle};

/// Stub for non-Windows platforms.
#[cfg(not(windows))]
pub mod stub {
    #[derive(Clone)]
    pub struct StatusOverlayHandle;

    impl StatusOverlayHandle {
        pub fn show_status(&self, _text: &str) {}
        #[allow(dead_code)]
        pub fn hide(&self) {}
        pub fn destroy(&self) {}
    }

    pub fn start_status_overlay() -> StatusOverlayHandle {
        StatusOverlayHandle
    }
}

#[cfg(not(windows))]
pub use stub::{start_status_overlay, StatusOverlayHandle};
