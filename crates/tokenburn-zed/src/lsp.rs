//! The language server: Zed's way to get text into its status bar.
//!
//! Zed extensions cannot add status-bar items, but Zed shows a language server's
//! work-done progress at the bottom of the window. So this server opens one
//! progress (`window/workDoneProgress/create`, then `$/progress` *begin*) titled
//! `TokenBurn` and keeps it open, sending a `report` with the new line whenever it
//! changes. Alerts become `window/showMessage` notifications.
//!
//! Interaction, since extensions cannot add commands either, is through code
//! actions (`cmd-.` / `ctrl-.` in any buffer): open the report, cycle the period,
//! pause/resume. They run as `workspace/executeCommand`.
//!
//! The server never touches documents: no diagnostics, no edits. It is registered
//! for many languages only so Zed starts it whatever file is open.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::{select, unbounded, Receiver, RecvTimeoutError, Sender};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use serde_json::{json, Value};
use tokenburn_core::Level;

use crate::budget::Notice;
use crate::config::Config;
use crate::engine::{self, Snapshot};
use crate::span::Span;

/// The progress title: Zed shows `TokenBurn: <line>`.
pub const TITLE: &str = "TokenBurn";

pub const CMD_REPORT: &str = "tokenburn.openReport";
pub const CMD_CYCLE: &str = "tokenburn.cyclePeriod";
pub const CMD_PAUSE: &str = "tokenburn.togglePause";
pub const CMD_REFRESH: &str = "tokenburn.refresh";
pub const CMD_GUI: &str = "tokenburn.openDesktopDashboard";
pub const CMD_WEB: &str = "tokenburn.openWebDashboard";
pub const COMMANDS: [&str; 6] = [
    CMD_REPORT,
    CMD_GUI,
    CMD_WEB,
    CMD_CYCLE,
    CMD_PAUSE,
    CMD_REFRESH,
];

/// Computes a snapshot for a config (the real one reads the logs).
pub type Refresher = Arc<dyn Fn(&Config) -> Snapshot + Send + Sync>;
/// Writes the report for a config and returns its path.
pub type ReportWriter = Arc<dyn Fn(&Config) -> Result<PathBuf, String> + Send + Sync>;

/// Is a dashboard installed (`true` = the web one)?
pub type Availability = Arc<dyn Fn(&Config, bool) -> bool + Send + Sync>;
/// Start the desktop dashboard.
pub type GuiLauncher = Arc<dyn Fn(&Config) -> Result<(), String> + Send + Sync>;
/// Make sure the web dashboard runs: its URL and the process if one was started.
pub type WebLauncher =
    Arc<dyn Fn(&Config) -> Result<(String, Option<Child>), String> + Send + Sync>;

/// Starts the dashboards (the real ones spawn `tokenburn-gui` / `tokenburn-web`).
#[derive(Clone)]
pub struct Dashboards {
    /// Is the dashboard installed? (decides whether its code action is offered)
    pub available: Availability,
    /// Start the desktop app.
    pub gui: GuiLauncher,
    /// Make sure the web dashboard runs; its URL and the process if one was started.
    pub web: WebLauncher,
}

impl Dashboards {
    /// `tokenburn-gui` and `tokenburn-web` from PATH, `~/.cargo/bin` or the settings.
    pub fn real() -> Self {
        use crate::dashboard;
        Self {
            available: Arc::new(|cfg: &Config, web: bool| {
                if web {
                    dashboard::locate(dashboard::WEB, cfg.dashboard.web_path.as_deref()).is_some()
                        || dashboard::is_tokenburn_web(cfg.dashboard.port)
                } else {
                    dashboard::locate(dashboard::GUI, cfg.dashboard.gui_path.as_deref()).is_some()
                }
            }),
            gui: Arc::new(|cfg: &Config| dashboard::open_gui(cfg).map(|_| ())),
            web: Arc::new(|cfg: &Config| dashboard::ensure_web(cfg, Duration::from_secs(15))),
        }
    }
}

/// Messages from the background threads to the main loop.
#[derive(Debug)]
pub enum Event {
    Snapshot(Snapshot),
    Report(Result<PathBuf, String>),
    /// The web dashboard is up (URL, and the process if this server started it).
    Web(Result<(String, Option<Child>), String>),
    /// Something the user asked for failed; shown as a warning.
    Failed(String),
}

/// Messages from the main loop to the refresh thread.
#[derive(Debug)]
enum Cmd {
    Config(Box<Config>),
    Refresh,
    Stop,
}

/// Run the refresh loop: a snapshot now, then every `refresh_secs`, or at once
/// after a config change. While disabled it waits without reading anything.
fn refresh_loop(mut cfg: Config, cmds: Receiver<Cmd>, events: Sender<Event>, refresh: Refresher) {
    loop {
        let snap = refresh(&cfg);
        if events.send(Event::Snapshot(snap)).is_err() {
            return;
        }
        let next = if cfg.enabled {
            cmds.recv_timeout(Duration::from_secs(cfg.refresh_secs))
        } else {
            cmds.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match next {
            Ok(Cmd::Config(c)) => cfg = *c,
            Ok(Cmd::Refresh) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Cmd::Stop) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// The status-bar progress.
#[derive(Debug, Default)]
struct Progress {
    /// The client accepts server-initiated progress.
    supported: bool,
    /// Counter for unique tokens (a token is dead after `end`).
    seq: u32,
    /// The `create` request in flight, with its token.
    creating: Option<(RequestId, String)>,
    /// The token of the open progress.
    active: Option<String>,
    /// Last text sent.
    shown: Option<String>,
    /// Text to show once the progress exists.
    wanted: Option<String>,
}

/// Runtime overrides from the code actions (cleared when Zed's settings change).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Overrides {
    period: Option<Span>,
    paused: Option<bool>,
}

pub struct Server {
    sender: Sender<Message>,
    /// Settings from `initializationOptions` (lower precedence).
    init_options: Value,
    /// Settings from `workspace/didChangeConfiguration`.
    settings: Value,
    overrides: Overrides,
    progress: Progress,
    next_id: i32,
    cmds: Sender<Cmd>,
    events_tx: Sender<Event>,
    report: ReportWriter,
    dashboards: Dashboards,
    /// A web dashboard this server started; stopped when the server exits.
    web_child: Option<Child>,
    logged: HashSet<String>,
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(mut c) = self.web_child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Percent-encode a path as a `file://` URI (forward slashes, `C:` kept).
pub fn file_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl Server {
    /// The effective config: settings layers plus the code-action overrides.
    pub fn config(&self) -> Config {
        let mut c = Config::from_layers([&self.init_options, &self.settings]);
        if let Some(p) = self.overrides.period {
            c.period = p;
        }
        if let Some(paused) = self.overrides.paused {
            c.enabled = !paused;
        }
        c
    }

    fn send(&self, m: impl Into<Message>) {
        let _ = self.sender.send(m.into());
    }

    fn notify(&self, method: &str, params: Value) {
        self.send(Notification::new(method.into(), params));
    }

    fn request(&mut self, method: &str, params: Value) -> RequestId {
        self.next_id += 1;
        let id = RequestId::from(format!("tokenburn-{}", self.next_id));
        self.send(Request::new(id.clone(), method.into(), params));
        id
    }

    fn config_changed(&mut self) {
        let _ = self.cmds.send(Cmd::Config(Box::new(self.config())));
    }

    fn show_message(&self, level: Level, text: &str) {
        let typ = match level {
            Level::Over => 1, // Error
            Level::Warn => 2, // Warning
            Level::Ok => 3,   // Info
        };
        self.notify(
            "window/showMessage",
            json!({ "type": typ, "message": text }),
        );
    }

    fn log(&self, text: &str) {
        self.notify("window/logMessage", json!({ "type": 3, "message": text }));
    }

    // ── status bar ───────────────────────────────────────────────────────────

    fn set_status(&mut self, text: Option<String>) {
        if !self.progress.supported {
            return;
        }
        self.progress.wanted = text.clone();
        match (text, self.progress.active.clone()) {
            (None, Some(token)) => {
                self.progress.active = None;
                self.progress.shown = None;
                self.notify(
                    "$/progress",
                    json!({ "token": token, "value": { "kind": "end" } }),
                );
            }
            (None, None) => {}
            (Some(t), Some(token)) => {
                if self.progress.shown.as_deref() != Some(t.as_str()) {
                    self.progress.shown = Some(t.clone());
                    self.notify(
                        "$/progress",
                        json!({ "token": token, "value": { "kind": "report", "message": t } }),
                    );
                }
            }
            (Some(_), None) => {
                if self.progress.creating.is_none() {
                    self.progress.seq += 1;
                    let token = format!("tokenburn/status/{}", self.progress.seq);
                    let id = self.request(
                        "window/workDoneProgress/create",
                        json!({ "token": token.clone() }),
                    );
                    self.progress.creating = Some((id, token));
                }
            }
        }
    }

    fn on_progress_created(&mut self, token: String, ok: bool) {
        if !ok {
            // The client refused: don't keep asking.
            self.progress.supported = false;
            self.log("TokenBurn: the client refused work-done progress; no status-bar text.");
            return;
        }
        match self.progress.wanted.clone() {
            Some(text) => {
                self.notify(
                    "$/progress",
                    json!({ "token": token.clone(), "value": {
                        "kind": "begin", "title": TITLE, "message": text.clone(), "cancellable": false
                    }}),
                );
                self.progress.active = Some(token);
                self.progress.shown = Some(text);
            }
            // Paused while the create was in flight: nothing to show.
            None => self.notify(
                "$/progress",
                json!({ "token": token, "value": { "kind": "end" } }),
            ),
        }
    }

    // ── events ───────────────────────────────────────────────────────────────

    pub fn on_event(&mut self, ev: Event) {
        match ev {
            Event::Snapshot(snap) => {
                self.set_status(snap.status);
                for n in &snap.notices {
                    self.show_message(n.level, &notice_text(n));
                }
                for w in snap.warnings {
                    if self.logged.insert(w.clone()) {
                        self.log(&format!("TokenBurn: {w}"));
                    }
                }
            }
            Event::Report(Ok(path)) => {
                self.request(
                    "window/showDocument",
                    json!({ "uri": file_uri(&path), "takeFocus": true }),
                );
            }
            Event::Report(Err(e)) => self.show_message(
                Level::Warn,
                &format!("TokenBurn: could not write the report: {e}"),
            ),
            Event::Web(Ok((url, child))) => {
                if let Some(c) = child {
                    if let Some(mut old) = self.web_child.replace(c) {
                        let _ = old.kill();
                    }
                }
                self.request(
                    "window/showDocument",
                    json!({ "uri": url, "external": true }),
                );
            }
            Event::Web(Err(e)) | Event::Failed(e) => {
                self.show_message(Level::Warn, &format!("TokenBurn: {e}"))
            }
        }
    }

    /// Handle one message from the client. Returns `false` on `exit`.
    pub fn on_message(&mut self, msg: Message) -> bool {
        match msg {
            Message::Request(req) => self.on_request(req),
            Message::Notification(n) => match n.method.as_str() {
                "exit" => return false,
                "workspace/didChangeConfiguration" => {
                    self.settings = n.params.get("settings").cloned().unwrap_or(Value::Null);
                    self.overrides = Overrides::default();
                    self.config_changed();
                }
                _ => {}
            },
            Message::Response(resp) => {
                if let Some((id, token)) = self.progress.creating.clone() {
                    if id == resp.id {
                        self.progress.creating = None;
                        let ok = resp.response_result.is_ok();
                        self.on_progress_created(token, ok);
                    }
                }
            }
        }
        true
    }

    fn on_request(&mut self, req: Request) {
        let result = match req.method.as_str() {
            "shutdown" => {
                let _ = self.cmds.send(Cmd::Stop);
                Ok(Value::Null)
            }
            "textDocument/codeAction" => Ok(self.code_actions(&req.params)),
            "workspace/executeCommand" => self.execute(&req.params),
            _ => Err((
                ErrorCode::MethodNotFound,
                format!("unhandled method {}", req.method),
            )),
        };
        let resp = match result {
            Ok(v) => Response::new_ok(req.id, v),
            Err((code, msg)) => Response::new_err(req.id, code as i32, msg),
        };
        self.send(resp);
    }

    /// The code actions: plain commands, offered only where `code_actions` allows
    /// (the first line by default) and only when the client asks for all kinds
    /// (never for format-on-save style `only` requests).
    pub fn code_actions(&self, params: &Value) -> Value {
        let cfg = self.config();
        let restricted = params
            .pointer("/context/only")
            .is_some_and(|o| !o.is_null());
        let line = params
            .pointer("/range/start/line")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if !cfg.code_actions.offered_at(line) || restricted {
            return json!([]);
        }
        let cmd = |title: String, command: &str| json!({ "title": title, "command": command });
        let mut out = vec![cmd("🔥 TokenBurn: open usage report".into(), CMD_REPORT)];
        if (self.dashboards.available)(&cfg, false) {
            out.push(cmd(
                "🔥 TokenBurn: open the desktop dashboard".into(),
                CMD_GUI,
            ));
        }
        if (self.dashboards.available)(&cfg, true) {
            out.push(cmd("🔥 TokenBurn: open the web dashboard".into(), CMD_WEB));
        }
        if cfg.enabled {
            out.push(cmd(
                format!(
                    "🔥 TokenBurn: status bar → {} (now {})",
                    cfg.period.next(),
                    cfg.period
                ),
                CMD_CYCLE,
            ));
            out.push(cmd("🔥 TokenBurn: pause the status bar".into(), CMD_PAUSE));
        } else {
            out.push(cmd("🔥 TokenBurn: resume the status bar".into(), CMD_PAUSE));
        }
        Value::Array(out)
    }

    fn execute(&mut self, params: &Value) -> Result<Value, (ErrorCode, String)> {
        let command = params.get("command").and_then(Value::as_str).unwrap_or("");
        match command {
            CMD_REPORT => {
                let cfg = self.config();
                let writer = self.report.clone();
                let tx = self.events_tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Event::Report(writer(&cfg)));
                });
            }
            CMD_GUI => {
                let cfg = self.config();
                let gui = self.dashboards.gui.clone();
                let tx = self.events_tx.clone();
                std::thread::spawn(move || {
                    if let Err(e) = gui(&cfg) {
                        let _ = tx.send(Event::Failed(e));
                    }
                });
            }
            CMD_WEB => {
                let cfg = self.config();
                let web = self.dashboards.web.clone();
                let tx = self.events_tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Event::Web(web(&cfg)));
                });
            }
            CMD_CYCLE => {
                self.overrides.period = Some(self.config().period.next());
                self.config_changed();
            }
            CMD_PAUSE => {
                self.overrides.paused = Some(self.config().enabled);
                self.config_changed();
            }
            CMD_REFRESH => {
                let _ = self.cmds.send(Cmd::Refresh);
            }
            other => {
                return Err((
                    ErrorCode::InvalidParams,
                    format!("unknown command {other:?}"),
                ));
            }
        }
        Ok(Value::Null)
    }
}

/// `TokenBurn: daily limit exceeded — Spent $21.00 of $20.00 today (105%)`.
pub fn notice_text(n: &Notice) -> String {
    format!("{} — {}", n.title, n.body)
}

/// The `initialize` result.
pub fn capabilities() -> Value {
    json!({
        "capabilities": {
            "codeActionProvider": true,
            "executeCommandProvider": { "commands": COMMANDS },
        },
        "serverInfo": { "name": "tokenburn-zed", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// Serve on `conn` until the client exits.
pub fn serve(
    conn: Connection,
    refresh: Refresher,
    report: ReportWriter,
    dashboards: Dashboards,
) -> Result<()> {
    let (id, params) = conn.initialize_start()?;
    conn.initialize_finish(id, capabilities())?;

    let (cmds_tx, cmds_rx) = unbounded();
    let (events_tx, events_rx) = unbounded();
    let mut server = Server {
        sender: conn.sender.clone(),
        init_options: params
            .get("initializationOptions")
            .cloned()
            .unwrap_or(Value::Null),
        settings: Value::Null,
        overrides: Overrides::default(),
        progress: Progress {
            supported: params
                .pointer("/capabilities/window/workDoneProgress")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            ..Default::default()
        },
        next_id: 0,
        cmds: cmds_tx,
        events_tx: events_tx.clone(),
        report,
        dashboards,
        web_child: None,
        logged: HashSet::new(),
    };
    if !server.progress.supported {
        server.log("TokenBurn: the client has no work-done progress; no status-bar text.");
    }

    let first = server.config();
    let worker = std::thread::spawn(move || refresh_loop(first, cmds_rx, events_tx, refresh));

    loop {
        select! {
            recv(conn.receiver) -> msg => match msg {
                Ok(msg) => if !server.on_message(msg) { break },
                Err(_) => break,
            },
            recv(events_rx) -> ev => if let Ok(ev) = ev { server.on_event(ev) },
        }
    }
    let _ = server.cmds.send(Cmd::Stop);
    drop(server);
    let _ = worker.join();
    Ok(())
}

/// A connection over `input` (already buffered, possibly peeked into) and stdout.
///
/// `lsp_server::Connection::stdio` owns stdin itself; this variant exists so
/// `tokenburn-zed` without arguments can look at the first byte to tell LSP from
/// MCP and still hand every byte to the server.
pub fn connection(mut input: impl std::io::BufRead + Send + 'static) -> Connection {
    let (to_server, from_client) = crossbeam_channel::bounded::<Message>(0);
    let (to_client, from_server) = crossbeam_channel::bounded::<Message>(0);
    std::thread::spawn(move || {
        while let Ok(Some(msg)) = Message::read(&mut input) {
            let exit = matches!(&msg, Message::Notification(n) if n.method == "exit");
            if to_server.send(msg).is_err() || exit {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        let mut out = std::io::stdout().lock();
        for msg in from_server {
            if msg.write(&mut out).is_err() {
                break;
            }
        }
    });
    Connection {
        sender: to_client,
        receiver: from_client,
    }
}

/// The real thing, reading `input` and writing stdout.
pub fn run(input: impl std::io::BufRead + Send + 'static) -> Result<()> {
    let refresh: Refresher = Arc::new(engine::refresh);
    let report: ReportWriter = Arc::new(|cfg: &Config| crate::write_report(cfg));
    serve(connection(input), refresh, report, Dashboards::real())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokenburn_core::ToolFilter;

    const T: Duration = Duration::from_secs(5);

    /// A client connected to a running server with fake data.
    struct Harness {
        client: Connection,
        thread: Option<std::thread::JoinHandle<()>>,
        refreshes: Arc<AtomicUsize>,
        logs: std::cell::RefCell<Vec<String>>,
    }

    impl Harness {
        fn start(progress: bool, init: Value) -> Self {
            let (server, client) = Connection::memory();
            let refreshes = Arc::new(AtomicUsize::new(0));
            let count = refreshes.clone();
            let refresh: Refresher = Arc::new(move |cfg: &Config| {
                count.fetch_add(1, Ordering::SeqCst);
                if !cfg.enabled {
                    return Snapshot::paused();
                }
                Snapshot {
                    status: Some(format!("🔥 42 [{}]", cfg.period)),
                    level: Level::Ok,
                    notices: Vec::new(),
                    warnings: vec!["zed: locked".into()],
                }
            });
            let report: ReportWriter =
                Arc::new(|_: &Config| Ok(PathBuf::from("/tmp/tb report.md")));
            let dashboards = Dashboards {
                available: Arc::new(|cfg: &Config, web: bool| web || cfg.tool == ToolFilter::All),
                gui: Arc::new(|cfg: &Config| {
                    if cfg.period == Span::All {
                        Err("tokenburn-gui is not installed — `cargo install tokenburn-gui`".into())
                    } else {
                        Ok(())
                    }
                }),
                web: Arc::new(|cfg: &Config| Ok((crate::dashboard::web_url(4321, cfg), None))),
            };
            let thread =
                std::thread::spawn(move || serve(server, refresh, report, dashboards).unwrap());
            client
                .sender
                .send(
                    Request::new(
                        1.into(),
                        "initialize".into(),
                        json!({
                            "capabilities": { "window": { "workDoneProgress": progress } },
                            "initializationOptions": init,
                        }),
                    )
                    .into(),
                )
                .unwrap();
            let h = Harness {
                client,
                thread: Some(thread),
                refreshes,
                logs: Default::default(),
            };
            let init = h.recv();
            assert!(matches!(init, Message::Response(ref r) if r.id == 1.into()));
            h.notify("initialized", json!({}));
            h
        }

        fn recv(&self) -> Message {
            self.client
                .receiver
                .recv_timeout(T)
                .expect("a message from the server")
        }

        /// Next message that is not a log line.
        fn recv_skip_logs(&self) -> Message {
            loop {
                match self.recv() {
                    Message::Notification(n) if n.method == "window/logMessage" => {
                        let text = n.params["message"].as_str().unwrap_or("").to_string();
                        self.logs.borrow_mut().push(text);
                    }
                    m => return m,
                }
            }
        }

        fn notify(&self, method: &str, params: Value) {
            self.client
                .sender
                .send(Notification::new(method.into(), params).into())
                .unwrap();
        }

        fn request(&self, id: i32, method: &str, params: Value) -> Value {
            self.client
                .sender
                .send(Request::new(id.into(), method.into(), params).into())
                .unwrap();
            match self.recv_skip_logs() {
                Message::Response(r) if r.id == id.into() => {
                    r.response_result.unwrap_or_else(|e| panic!("error: {e:?}"))
                }
                other => panic!("expected the response to {method}, got {other:?}"),
            }
        }

        /// Accept a progress `create` and return its token.
        fn accept_create(&self) -> String {
            match self.recv_skip_logs() {
                Message::Request(r) if r.method == "window/workDoneProgress/create" => {
                    let token = r.params["token"].as_str().unwrap().to_string();
                    self.client
                        .sender
                        .send(Response::new_ok(r.id, Value::Null).into())
                        .unwrap();
                    token
                }
                other => panic!("expected create, got {other:?}"),
            }
        }

        fn progress(&self) -> Value {
            match self.recv_skip_logs() {
                Message::Notification(n) if n.method == "$/progress" => n.params,
                other => panic!("expected $/progress, got {other:?}"),
            }
        }

        fn stop(mut self) {
            let _ = self.request(99, "shutdown", Value::Null);
            self.notify("exit", Value::Null);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    #[test]
    fn status_appears_as_a_progress_and_updates() {
        let h = Harness::start(true, json!({ "period": "week" }));
        let token = h.accept_create();
        let begin = h.progress();
        assert_eq!(begin["token"], token);
        assert_eq!(begin["value"]["kind"], "begin");
        assert_eq!(begin["value"]["title"], TITLE);
        assert_eq!(
            begin["value"]["message"], "🔥 42 [week]",
            "init options apply"
        );

        // Settings from Zed win over the init options and refresh at once.
        h.notify(
            "workspace/didChangeConfiguration",
            json!({ "settings": { "period": "month" } }),
        );
        let report = h.progress();
        assert_eq!(report["value"]["kind"], "report");
        assert_eq!(report["value"]["message"], "🔥 42 [month]");
        h.stop();
    }

    #[test]
    fn warnings_are_logged_once() {
        let h = Harness::start(true, Value::Null);
        h.accept_create();
        let _ = h.progress();
        h.request(
            5,
            "workspace/executeCommand",
            json!({ "command": CMD_REFRESH }),
        );
        // Give the second refresh time to arrive, then drain everything.
        std::thread::sleep(Duration::from_millis(300));
        while let Ok(m) = h.client.receiver.try_recv() {
            if let Message::Notification(n) = m {
                if n.method == "window/logMessage" {
                    h.logs.borrow_mut().push(n.params["message"].to_string());
                }
            }
        }
        assert!(h.refreshes.load(Ordering::SeqCst) >= 2);
        let logs = h.logs.borrow().clone();
        let locked: Vec<_> = logs.iter().filter(|l| l.contains("zed: locked")).collect();
        assert_eq!(locked.len(), 1, "{logs:?}");
        h.stop();
    }

    #[test]
    fn code_actions_cycle_and_pause() {
        let h = Harness::start(true, Value::Null);
        let token = h.accept_create();
        assert_eq!(h.progress()["value"]["message"], "🔥 42 [today]");

        let doc = json!({ "textDocument": { "uri": "file:///a.rs" },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            "context": { "diagnostics": [] } });
        let actions = h.request(2, "textDocument/codeAction", doc.clone());
        let titles: Vec<&str> = actions
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles.len(), 5, "{titles:?}");
        assert!(titles[1].contains("desktop dashboard"));
        assert!(titles[2].contains("web dashboard"));
        assert!(titles[3].contains("→ week (now today)"));

        h.request(
            3,
            "workspace/executeCommand",
            json!({ "command": CMD_CYCLE }),
        );
        assert_eq!(h.progress()["value"]["message"], "🔥 42 [week]");

        h.request(
            4,
            "workspace/executeCommand",
            json!({ "command": CMD_PAUSE }),
        );
        let end = h.progress();
        assert_eq!(
            (end["token"].as_str(), end["value"]["kind"].as_str()),
            (Some(token.as_str()), Some("end"))
        );
        let actions = h.request(5, "textDocument/codeAction", doc);
        assert_eq!(actions.as_array().unwrap().len(), 4);
        assert!(actions[3]["title"].as_str().unwrap().contains("resume"));

        // Resume: a fresh token, since the old one ended.
        h.request(
            6,
            "workspace/executeCommand",
            json!({ "command": CMD_PAUSE }),
        );
        let token2 = h.accept_create();
        assert_ne!(token, token2);
        assert_eq!(h.progress()["value"]["kind"], "begin");
        h.stop();
    }

    #[test]
    fn restricted_code_action_requests_get_nothing() {
        let h = Harness::start(true, Value::Null);
        h.accept_create();
        let _ = h.progress();
        let only = json!({ "textDocument": { "uri": "file:///a.rs" },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            "context": { "diagnostics": [], "only": ["source.organizeImports"] } });
        assert_eq!(h.request(2, "textDocument/codeAction", only), json!([]));
        h.stop();
    }

    #[test]
    fn code_actions_can_be_switched_off() {
        let h = Harness::start(true, json!({ "code_actions": false }));
        h.accept_create();
        let _ = h.progress();
        let doc = json!({ "context": { "diagnostics": [] } });
        assert_eq!(h.request(2, "textDocument/codeAction", doc), json!([]));
        h.stop();
    }

    #[test]
    fn code_actions_live_on_the_first_line_by_default() {
        let h = Harness::start(true, Value::Null);
        h.accept_create();
        let _ = h.progress();
        let at = |line: u64| {
            json!({ "textDocument": { "uri": "file:///a.rs" },
            "range": { "start": { "line": line, "character": 0 }, "end": { "line": line, "character": 0 } },
            "context": { "diagnostics": [] } })
        };
        assert_eq!(
            h.request(2, "textDocument/codeAction", at(0))
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(h.request(3, "textDocument/codeAction", at(7)), json!([]));
        h.notify(
            "workspace/didChangeConfiguration",
            json!({ "settings": { "code_actions": "everywhere" } }),
        );
        // The setting applies to the next request.
        assert_eq!(
            h.request(4, "textDocument/codeAction", at(7))
                .as_array()
                .unwrap()
                .len(),
            5
        );
        h.stop();
    }

    #[test]
    fn open_report_shows_the_document() {
        let h = Harness::start(true, Value::Null);
        h.accept_create();
        let _ = h.progress();
        h.request(
            2,
            "workspace/executeCommand",
            json!({ "command": CMD_REPORT }),
        );
        match h.recv_skip_logs() {
            Message::Request(r) => {
                assert_eq!(r.method, "window/showDocument");
                assert_eq!(r.params["uri"], "file:///tmp/tb%20report.md");
                assert_eq!(r.params["takeFocus"], true);
            }
            other => panic!("{other:?}"),
        }
        h.stop();
    }

    #[test]
    fn dashboards_open_in_the_browser_or_report_why_not() {
        let h = Harness::start(true, json!({ "period": "month", "tool": "pi" }));
        h.accept_create();
        let _ = h.progress();
        // The GUI is "missing" for tool=pi in this fake, so no action is offered…
        let doc = json!({ "context": { "diagnostics": [] } });
        let titles = h.request(2, "textDocument/codeAction", doc);
        assert!(
            !titles.to_string().contains("desktop dashboard"),
            "{titles}"
        );
        // …and the web dashboard opens externally on the status bar's view.
        h.request(3, "workspace/executeCommand", json!({ "command": CMD_WEB }));
        match h.recv_skip_logs() {
            Message::Request(r) => {
                assert_eq!(r.method, "window/showDocument");
                assert_eq!(r.params["external"], true);
                assert_eq!(
                    r.params["uri"],
                    "http://127.0.0.1:4321/?tool=pi&bucket=daily&window=month"
                );
            }
            other => panic!("{other:?}"),
        }
        // A failing GUI start becomes a warning, not a protocol error.
        h.notify(
            "workspace/didChangeConfiguration",
            json!({ "settings": { "period": "all" } }),
        );
        let _ = h.progress();
        h.request(4, "workspace/executeCommand", json!({ "command": CMD_GUI }));
        match h.recv_skip_logs() {
            Message::Notification(n) => {
                assert_eq!(n.method, "window/showMessage");
                assert!(n.params["message"]
                    .as_str()
                    .unwrap()
                    .contains("cargo install tokenburn-gui"));
            }
            other => panic!("{other:?}"),
        }
        h.stop();
    }

    #[test]
    fn unknown_commands_and_methods_are_errors() {
        let h = Harness::start(false, Value::Null);
        h.client
            .sender
            .send(
                Request::new(
                    7.into(),
                    "workspace/executeCommand".into(),
                    json!({ "command": "nope" }),
                )
                .into(),
            )
            .unwrap();
        h.client
            .sender
            .send(Request::new(8.into(), "textDocument/hover".into(), json!({})).into())
            .unwrap();
        let mut errors = 0;
        while errors < 2 {
            if let Message::Response(r) = h.recv_skip_logs() {
                assert!(r.response_result.is_err());
                errors += 1;
            }
        }
        h.stop();
    }

    #[test]
    fn without_progress_support_nothing_is_created() {
        let h = Harness::start(false, Value::Null);
        // The refresh still runs, but no create request is sent.
        let deadline = std::time::Instant::now() + Duration::from_millis(300);
        while std::time::Instant::now() < deadline {
            if let Ok(Message::Request(r)) =
                h.client.receiver.recv_timeout(Duration::from_millis(50))
            {
                panic!("unexpected request {}", r.method);
            }
        }
        assert!(h.refreshes.load(Ordering::SeqCst) >= 1);
        h.stop();
    }

    #[test]
    fn a_refused_create_disables_the_status() {
        let h = Harness::start(true, Value::Null);
        match h.recv_skip_logs() {
            Message::Request(r) => h
                .client
                .sender
                .send(Response::new_err(r.id, ErrorCode::InternalError as i32, "no".into()).into())
                .unwrap(),
            other => panic!("{other:?}"),
        }
        h.request(
            3,
            "workspace/executeCommand",
            json!({ "command": CMD_CYCLE }),
        );
        std::thread::sleep(Duration::from_millis(200));
        while let Ok(m) = h.client.receiver.try_recv() {
            assert!(
                !matches!(m, Message::Request(_)),
                "no second create after a refusal: {m:?}"
            );
        }
        h.stop();
    }

    #[test]
    fn file_uris_are_percent_encoded() {
        assert_eq!(file_uri(Path::new("/a b/r.md")), "file:///a%20b/r.md");
        assert_eq!(
            file_uri(Path::new("C:\\Users\\me\\r.md")),
            "file:///C:/Users/me/r.md"
        );
        assert_eq!(file_uri(Path::new("/ü")), "file:///%C3%BC");
    }

    #[test]
    fn notice_text_joins_title_and_body() {
        let n = Notice {
            level: Level::Over,
            title: "T".into(),
            body: "B".into(),
        };
        assert_eq!(notice_text(&n), "T — B");
    }
}
