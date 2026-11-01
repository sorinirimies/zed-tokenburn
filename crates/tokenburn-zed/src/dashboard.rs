//! Open tokenburn's own Rust dashboards from Zed.
//!
//! Zed extensions cannot draw UI, so instead of re-drawing charts here the code
//! actions start the real thing — same core, same 43 themes, same interactive
//! charts:
//!
//! * **`tokenburn-gui`** — the native Iced desktop app, started detached;
//! * **`tokenburn-web`** — the live Topcoat web dashboard. It is started on
//!   `127.0.0.1:<port>` unless something already listens there, and Zed opens it
//!   in the browser (`window/showDocument` with `external: true`).
//!
//! Both start on the status bar's tool and period. They come from
//! `cargo install tokenburn-gui tokenburn-web` (or the tokenburn releases); the
//! binaries are looked up on `PATH` and in `~/.cargo/bin`, or set in the settings.

use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokenburn_core::ToolFilter;

use crate::config::Config;
use crate::span::Span;

pub const GUI: &str = "tokenburn-gui";
pub const WEB: &str = "tokenburn-web";
pub const DEFAULT_PORT: u16 = 3000;

/// `dashboard` settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DashboardConfig {
    /// Explicit path of `tokenburn-gui` (default: looked up).
    pub gui_path: Option<PathBuf>,
    /// Explicit path of `tokenburn-web` (default: looked up).
    pub web_path: Option<PathBuf>,
    /// Port of the web dashboard.
    pub port: u16,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            gui_path: None,
            web_path: None,
            port: DEFAULT_PORT,
        }
    }
}

impl DashboardConfig {
    /// Overlay the keys of a `dashboard` object (lenient, like every setting).
    pub fn apply(&mut self, v: &Value) {
        let path = |k: &str| match v.get(k) {
            Some(Value::String(s)) if !s.trim().is_empty() => Some(Some(PathBuf::from(s.trim()))),
            Some(Value::Null) => Some(None),
            _ => None,
        };
        if let Some(p) = path("gui_path") {
            self.gui_path = p;
        }
        if let Some(p) = path("web_path") {
            self.web_path = p;
        }
        if let Some(port) = v
            .get("port")
            .and_then(Value::as_u64)
            .filter(|p| (1024..=65535).contains(p))
        {
            self.port = port as u16;
        }
    }
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Find `name` in the directories of `path_var`, then in `~/.cargo/bin` (Zed may
/// start servers with a PATH that misses it). `explicit` wins when it exists.
pub fn find(
    name: &str,
    explicit: Option<&Path>,
    path_var: Option<OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return p.is_file().then(|| p.to_path_buf());
    }
    let exe = exe_name(name);
    let dirs = path_var
        .as_deref()
        .map(|p| std::env::split_paths(p).collect::<Vec<_>>())
        .unwrap_or_default();
    dirs.into_iter()
        .chain(home.map(|h| h.join(".cargo").join("bin")))
        .map(|d| d.join(&exe))
        .find(|p| p.is_file())
}

/// [`find`] with the real environment.
pub fn locate(name: &str, explicit: Option<&Path>) -> Option<PathBuf> {
    find(name, explicit, std::env::var_os("PATH"), dirs::home_dir())
}

/// Bucket flag, window and web bucket for a span: the last hour and today are
/// hourly, week and month daily, year and all time monthly.
fn view_of(span: Span) -> (&'static str, &'static str, &'static str) {
    match span {
        Span::Hour | Span::Today => ("--hourly", "today", "hourly"),
        Span::Week | Span::Month => ("--daily", "month", "daily"),
        Span::Year | Span::All => ("--monthly", "all", "monthly"),
    }
}

/// Arguments for `tokenburn-gui`: the status bar's tool and a matching view.
pub fn gui_args(cfg: &Config) -> Vec<String> {
    let (flag, _, _) = view_of(cfg.period);
    let mut args = vec!["--tool".into(), cfg.tool.as_str().into(), flag.into()];
    if matches!(cfg.period, Span::Year | Span::All) {
        args.push("--all".into());
    }
    // No `--currency`: tokenburn-gui would save it as everyone's display currency.
    args
}

/// The web dashboard URL for the status bar's tool and period.
pub fn web_url(port: u16, cfg: &Config) -> String {
    let (_, window, bucket) = view_of(cfg.period);
    let tool = match cfg.tool {
        ToolFilter::All => "all",
        ToolFilter::Only(t) => t.as_str(),
    };
    format!("http://127.0.0.1:{port}/?tool={tool}&bucket={bucket}&window={window}")
}

/// Something accepts connections on `127.0.0.1:port`.
pub fn port_open(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
}

/// `GET path` on `127.0.0.1:port` over plain HTTP/1.0; the body on a 200.
fn http_get(port: u16, path: &str) -> Option<String> {
    use std::io::{Read, Write};
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(300)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    s.set_write_timeout(Some(Duration::from_secs(2))).ok()?;
    // One write: a request split over several segments may be answered and
    // closed before the rest arrives, which resets the connection.
    let req = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).ok()?;
    let mut buf = Vec::new();
    s.take(4 * 1024 * 1024).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status_ok = head
        .lines()
        .next()
        .is_some_and(|l| l.split_whitespace().nth(1) == Some("200"));
    status_ok.then(|| body.to_string())
}

/// Does `body` (of `/api/themes`) come from tokenburn-web? It lists every theme,
/// the first one being `tokenburn-core`'s first preset.
pub fn looks_like_tokenburn_themes(body: &str) -> bool {
    serde_json::from_str::<Value>(body).is_ok_and(|v| {
        v.get(0).and_then(|t| t.get("name")).and_then(Value::as_str)
            == tokenburn_core::THEME_NAMES.first().copied()
            && v[0].get("background").is_some()
    })
}

/// A tokenburn web dashboard (not some other program) listens on `port`.
pub fn is_tokenburn_web(port: u16) -> bool {
    http_get(port, "/api/themes").is_some_and(|b| looks_like_tokenburn_themes(&b))
}

fn not_installed(name: &str) -> String {
    format!(
        "{name} is not installed — `cargo install {name}` (or set dashboard.{}_path in the \
         tokenburn settings)",
        if name == GUI { "gui" } else { "web" }
    )
}

/// Start the desktop dashboard, detached. Nothing to wait for: Iced opens its
/// own window.
pub fn open_gui(cfg: &Config) -> Result<PathBuf, String> {
    let path = locate(GUI, cfg.dashboard.gui_path.as_deref()).ok_or_else(|| not_installed(GUI))?;
    Command::new(&path)
        .args(gui_args(cfg))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("starting {}: {e}", path.display()))?;
    Ok(path)
}

/// Make sure the web dashboard runs on the configured port: reuse one that is
/// already there, otherwise start `tokenburn-web --no-open` and wait (up to
/// `wait`) until it accepts connections. Returns the URL to open and the child
/// if this call started one (the language server stops it on exit).
pub fn ensure_web(cfg: &Config, wait: Duration) -> Result<(String, Option<Child>), String> {
    let port = cfg.dashboard.port;
    let url = web_url(port, cfg);
    if port_open(port) {
        return if is_tokenburn_web(port) {
            Ok((url, None))
        } else {
            Err(format!(
                "port {port} is used by another program; set dashboard.port in the tokenburn settings"
            ))
        };
    }
    let path = locate(WEB, cfg.dashboard.web_path.as_deref()).ok_or_else(|| not_installed(WEB))?;
    let mut child = Command::new(&path)
        .args(["--no-open", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("starting {}: {e}", path.display()))?;
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if is_tokenburn_web(port) {
            return Ok((url, Some(child)));
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!("{} exited early ({status})", path.display()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    Err(format!(
        "{} did not start listening on port {port}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(v: Value) -> Config {
        Config::from_layers([&v])
    }

    #[test]
    fn dashboard_settings_are_lenient() {
        let mut d = DashboardConfig::default();
        d.apply(&json!({ "gui_path": "/opt/tb/tokenburn-gui", "port": 4000 }));
        assert_eq!(d.gui_path, Some(PathBuf::from("/opt/tb/tokenburn-gui")));
        assert_eq!(d.port, 4000);
        d.apply(&json!({ "gui_path": null, "port": 80, "web_path": 7 }));
        assert_eq!(d.gui_path, None, "null clears");
        assert_eq!(d.port, 4000, "privileged ports are refused");
        assert_eq!(d.web_path, None, "a non-string is ignored");
        d.apply(&json!({ "web_path": "  " }));
        assert_eq!(d.web_path, None, "blank is ignored");
    }

    #[test]
    fn the_config_reads_the_dashboard_object() {
        let c = cfg(json!({ "dashboard": { "port": 3999 } }));
        assert_eq!(c.dashboard.port, 3999);
        assert_eq!(Config::default().dashboard, DashboardConfig::default());
    }

    #[test]
    fn gui_starts_on_the_status_bar_view() {
        assert_eq!(gui_args(&Config::default()), ["--tool", "all", "--hourly"]);
        let c = cfg(json!({ "period": "all", "tool": "claude", "currency": "EUR" }));
        assert_eq!(
            gui_args(&c),
            ["--tool", "claude", "--monthly", "--all"],
            "the Zed currency is not forced on the shared settings"
        );
        assert_eq!(gui_args(&cfg(json!({ "period": "week" })))[2], "--daily");
    }

    #[test]
    fn web_url_carries_tool_bucket_and_window() {
        assert_eq!(
            web_url(3000, &Config::default()),
            "http://127.0.0.1:3000/?tool=all&bucket=hourly&window=today"
        );
        let c = cfg(json!({ "period": "month", "tool": "pi" }));
        assert_eq!(
            web_url(8080, &c),
            "http://127.0.0.1:8080/?tool=pi&bucket=daily&window=month"
        );
    }

    #[test]
    fn find_prefers_explicit_then_path_then_cargo_bin() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        let home = dir.path().join("home");
        let cargo_bin = home.join(".cargo").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&cargo_bin).unwrap();
        let exe = exe_name(GUI);
        std::fs::write(cargo_bin.join(&exe), "").unwrap();

        // Only in ~/.cargo/bin.
        let path_var = std::env::join_paths([&bin]).unwrap();
        assert_eq!(
            find(GUI, None, Some(path_var.clone()), Some(home.clone())),
            Some(cargo_bin.join(&exe))
        );
        // PATH wins over ~/.cargo/bin.
        std::fs::write(bin.join(&exe), "").unwrap();
        assert_eq!(
            find(GUI, None, Some(path_var.clone()), Some(home.clone())),
            Some(bin.join(&exe))
        );
        // An explicit path wins when it exists, and is not second-guessed when it doesn't.
        let own = dir.path().join("own");
        std::fs::write(&own, "").unwrap();
        assert_eq!(
            find(GUI, Some(&own), Some(path_var.clone()), None),
            Some(own)
        );
        assert_eq!(
            find(GUI, Some(&dir.path().join("missing")), Some(path_var), None),
            None
        );
        assert_eq!(find(WEB, None, None, None), None);
    }

    #[test]
    fn port_open_sees_a_listener() {
        let l = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(port_open(port));
        drop(l);
        assert!(!port_open(port));
    }

    /// A one-shot HTTP server answering every request with `body`.
    fn serve_once(body: &'static str) -> u16 {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                // Read the whole request head before answering.
                let mut req = Vec::new();
                let mut b = [0u8; 256];
                while !req.ends_with(b"\r\n\r\n") {
                    match s.read(&mut b) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => req.extend_from_slice(&b[..n]),
                    }
                }
                let _ = write!(
                    s,
                    "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{body}"
                );
            }
        });
        port
    }

    #[test]
    fn tokenburn_web_is_recognised_by_its_theme_list() {
        let first = tokenburn_core::THEME_NAMES[0];
        let ok = format!(r##"[{{"name":"{first}","background":"#000000"}}]"##);
        assert!(looks_like_tokenburn_themes(&ok));
        assert!(!looks_like_tokenburn_themes(
            r##"[{"name":"Other","background":"#000"}]"##
        ));
        assert!(!looks_like_tokenburn_themes("<html>"));
        assert!(!looks_like_tokenburn_themes("[]"));
    }

    #[test]
    fn ensure_web_reuses_a_running_tokenburn_web_only() {
        let body: &'static str = Box::leak(
            format!(
                r##"[{{"name":"{}","background":"#000"}}]"##,
                tokenburn_core::THEME_NAMES[0]
            )
            .into_boxed_str(),
        );
        let port = serve_once(body);
        let c = cfg(json!({ "dashboard": { "port": port, "web_path": "/nonexistent" } }));
        let (url, child) = ensure_web(&c, Duration::from_secs(1)).unwrap();
        assert!(url.starts_with(&format!("http://127.0.0.1:{port}/")));
        assert!(child.is_none(), "nothing started");

        // Some other web app on the port is not mistaken for the dashboard.
        let other = serve_once("<html><title>Something else</title></html>");
        let c = cfg(json!({ "dashboard": { "port": other } }));
        let e = ensure_web(&c, Duration::from_secs(1)).unwrap_err();
        assert!(e.contains("used by another program"), "{e}");
    }

    #[test]
    fn missing_dashboards_say_how_to_install_them() {
        let c = cfg(json!({ "dashboard": {
            "port": 1, "gui_path": "/nonexistent/gui", "web_path": "/nonexistent/web" } }));
        let e = open_gui(&c).unwrap_err();
        assert!(e.contains("cargo install tokenburn-gui"), "{e}");
        // Port 1 is never open for us, so the web path is looked up and missing.
        let mut c = c;
        c.dashboard.port = 1;
        let e = ensure_web(&c, Duration::from_millis(100)).unwrap_err();
        assert!(e.contains("cargo install tokenburn-web"), "{e}");
    }
}
