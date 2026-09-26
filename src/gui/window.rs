//! The window: a system WebView (WebView2 on Windows, WebKitGTK elsewhere)
//! showing ui/index.html. The page posts JSON messages (`window.ipc`); rzr
//! answers by calling `rzr.state(...)` and `rzr.toast(...)` in the page.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, WindowBuilder};
use wry::http::{header::CONTENT_TYPE, Response, StatusCode};
use wry::{WebContext, WebViewBuilder};

use super::{assets, App, Msg};
use crate::dlog;
use crate::worker::Notify;

/// The page's --ink, so the window never flashes another colour.
const BG: (u8, u8, u8, u8) = (0x12, 0x15, 0x18, 0xFF);
/// Show the window even if the page never says it's ready.
const SHOW_ANYWAY: Duration = Duration::from_secs(3);

enum UserEvent {
    /// A worker thread has news.
    Wake,
    /// A message from the page.
    Page(String),
}

/// Open the panel. Returns only on failure; closing the window ends the process.
pub fn run(demo: bool) -> Result<(), String> {
    if !demo {
        crate::instance::mark_panel_open();
    }
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let wake = Mutex::new(event_loop.create_proxy());
    let notify: Notify = Arc::new(move || {
        if let Ok(p) = wake.lock() {
            let _ = p.send_event(UserEvent::Wake);
        }
    });

    let window = WindowBuilder::new()
        .with_title("rzr")
        .with_inner_size(LogicalSize::new(1140.0, 860.0))
        .with_min_inner_size(LogicalSize::new(940.0, 640.0))
        .with_window_icon(Icon::from_rgba(app_icon(), ICON_SIZE, ICON_SIZE).ok())
        .with_background_color(BG)
        // Shown once the page has drawn, so it never flashes white.
        .with_visible(false)
        .build(&event_loop)
        .map_err(|e| e.to_string())?;

    let mut web_context = WebContext::new(webview_data_dir());
    let ipc = Mutex::new(event_loop.create_proxy());
    let builder = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_custom_protocol("rzr".into(), |_id, request| serve(request.uri().path()))
        .with_ipc_handler(move |request| {
            if let Ok(p) = ipc.lock() {
                let _ = p.send_event(UserEvent::Page(request.into_body()));
            }
        })
        // Windows turns this into http://rzr.localhost/.
        .with_url("rzr://localhost/")
        .with_background_color(BG)
        .with_hotkeys_zoom(false)
        // The panel never navigates away (a dropped file would, otherwise).
        .with_navigation_handler(|url| url.starts_with("rzr://") || url.starts_with("http://rzr.localhost/"));

    #[cfg(windows)]
    let webview = {
        use wry::WebViewBuilderExtWindows;
        builder.with_browser_accelerator_keys(false).with_default_context_menus(false).build(&window)
    };
    #[cfg(not(windows))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().ok_or("GTK window without a container")?;
        builder.build_gtk(vbox)
    };
    let webview = webview.map_err(|e| e.to_string())?;
    dlog!("window: webview {}", wry::webview_version().unwrap_or_default());

    let mut app = App::new(demo, notify);
    let mut loaded = false;
    let show_by = Instant::now() + SHOW_ANYWAY;
    event_loop.run(move |event, _, flow| {
        let _keep = &web_context;
        match event {
            Event::UserEvent(UserEvent::Wake) => app.pump(),
            Event::UserEvent(UserEvent::Page(text)) => match serde_json::from_str::<Msg>(&text) {
                Ok(msg) => {
                    if matches!(msg, Msg::Ready) {
                        loaded = true;
                        window.set_visible(true);
                    }
                    app.handle(msg);
                }
                Err(e) => dlog!("invalid message from the page ({e}): {text}"),
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                app.shutdown();
                *flow = ControlFlow::Exit;
                return;
            }
            _ => {}
        }

        let now = Instant::now();
        if now >= app.next_tick() {
            app.tick(now);
        }
        if !loaded && now >= show_by && !window.is_visible() {
            dlog!("the page did not answer; showing the window anyway");
            window.set_visible(true);
        }
        // Until the page is ready its functions don't exist yet; it asks for
        // the state (Msg::Ready) and gets everything then.
        if loaded {
            if let Some(view) = app.take_view() {
                let _ = webview.evaluate_script(&format!("rzr.state({view})"));
            }
            for (text, error) in app.take_toasts() {
                let _ = webview.evaluate_script(&format!("rzr.toast({}, {error})", Value::from(text)));
            }
        }
        let wake_at = if loaded { app.next_tick() } else { app.next_tick().min(show_by) };
        *flow = ControlFlow::WaitUntil(wake_at);
    })
}

fn serve(path: &str) -> Response<std::borrow::Cow<'static, [u8]>> {
    match assets::get(path) {
        Some((content_type, body)) => Response::builder().header(CONTENT_TYPE, content_type).body(body),
        None => Response::builder().status(StatusCode::NOT_FOUND).body((&b"not found"[..]).into()),
    }
    .expect("static response")
}

/// WebView2 keeps its cache here instead of next to rzr.exe.
fn webview_data_dir() -> Option<std::path::PathBuf> {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join("rzr").join("webview"))
    } else {
        None
    }
}

const ICON_SIZE: u32 = 64;

/// Window icon: a mint ring on graphite, drawn procedurally.
fn app_icon() -> Vec<u8> {
    const N: u32 = ICON_SIZE;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    let c = N as f32 / 2.0 - 0.5;
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt() / (N as f32 / 2.0);
            let (r, g, b, a) = if d > 0.97 {
                (0, 0, 0, 0)
            } else if d > 0.6 || d < 0.28 {
                (0x3F, 0xD9, 0x8A, 255)
            } else {
                (0x12, 0x15, 0x18, 255)
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    rgba
}
