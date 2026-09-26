//! The page's files (ui/), built into the exe. With the RZR_UI_DIR
//! environment variable set to a folder, they are read from there instead,
//! so the page can be edited and reloaded (F5) without rebuilding.

use std::borrow::Cow;

const FILES: [(&str, &str); 3] = [
    ("index.html", include_str!("../../ui/index.html")),
    ("app.css", include_str!("../../ui/app.css")),
    ("app.js", include_str!("../../ui/app.js")),
];

fn content_type(name: &str) -> &'static str {
    match name.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// Look up a file by its URL path ("/" is index.html). Returns the content
/// type and the bytes.
pub fn get(path: &str) -> Option<(&'static str, Cow<'static, [u8]>)> {
    let name = match path.trim_start_matches('/') {
        "" => "index.html",
        name => name,
    };
    // Only plain file names: nothing outside the folder.
    if name.contains(['/', '\\']) || name.starts_with('.') {
        return None;
    }
    if let Some(dir) = std::env::var_os("RZR_UI_DIR") {
        return std::fs::read(std::path::Path::new(&dir).join(name))
            .ok()
            .map(|bytes| (content_type(name), Cow::Owned(bytes)));
    }
    FILES.iter().find(|(n, _)| *n == name).map(|(n, text)| (content_type(n), Cow::Borrowed(text.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_embedded_files_only() {
        assert_eq!(get("/").unwrap().0, "text/html; charset=utf-8");
        assert!(get("/app.js").is_some());
        assert!(get("/../Cargo.toml").is_none());
        assert!(get("/missing.txt").is_none());
    }
}
