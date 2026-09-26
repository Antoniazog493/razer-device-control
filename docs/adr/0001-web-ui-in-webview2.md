# 0001. The interface is a web page in WebView2

- Status: Accepted
- Date: 2026-09-26

## Context

The panel was drawn with egui. With OpenGL it showed black frames now and then on Windows; moved to Direct3D 12 (wgpu), `rzr.exe` grew from 5.8 to 8.7 MB. Shaping the interface in Rust code was slow, and the app is a simple panel that will grow (THX, Windows enhancements, maybe other models). Criteria: free, light, easy to design and extend.

## Decision

The interface is a web page (`ui/`: HTML, CSS and JavaScript without frameworks) shown in the system's WebView: WebView2 on Windows, through `wry` and `tao`. Rust is the controller: it sends the page the whole state as JSON and takes JSON commands (`Msg`). The page is built into the exe.

## Alternatives

- **Keep egui + Direct3D:** works, but 8.7 MB and the interface stays hard to shape.
- **egui with OpenGL only:** lighter, but the black frames come back.
- **Slint with CPU rendering:** free and declarative, but a test build came to ~8.5 MB (Unicode data and images it always includes).
- **Native Win32 controls:** the smallest exe, but a lot of work and hard to style.
- **WebView2** (chosen): a ~1.3 MB exe, designed with HTML/CSS, viewable in any browser with demo data, and no GPU handling of its own.

## Consequences

- **Size and GPU:** `rzr.exe` drops to ~1.3 MB and the GPU problems go away.
- **RAM:** with the panel open it uses about 80–120 MB (Edge). The background watcher doesn't load the page and doesn't pay that.
- **Requirement:** WebView2 ships with up-to-date Windows 10 and Windows 11; if it's missing, rzr says so with the installer's link.
- **Contract:** page and Rust talk through a contract (`Msg` and `App::view`) that must be kept, and `demo.js` must follow it.
- **Linux:** development on Linux needs WebKitGTK (`libwebkit2gtk-4.1-dev`).
