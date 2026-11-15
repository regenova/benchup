//! Agent connection: lets an AI agent on this computer edit the open project
//! through `benchup bridge` (see `crates/benchup-cli/src/bridge.rs`).
//!
//! A browser tab cannot listen on a port, so BenchUp connects out to the
//! bridge on `ws://127.0.0.1:<port>`, on the web and on the desktop. It
//! proves it knows the pairing token with a challenge-response (the token is
//! never sent) and checks that the bridge knows it too, then runs the calls it
//! receives and answers each with a result:
//! * bridge to app: `challenge {nonce, protocol}`, `ready {proof}`, `error`,
//!   `cancel {id}`, `call {id, name, arguments, client, deadline_ms}`;
//! * app to bridge: `authenticate` (sent by the socket), `prove {nonce,
//!   proof}`, `result {id, ok, result}`, `status {message}`.
//!
//! Every call that changes the model is one undo step, `Agent: …`, recorded
//! like the designer's own edits, so shared projects keep their read-only
//! state and edit leases. Calls run in arrival order, a few per frame, from
//! `App::logic`, so a minimized window or hidden tab still answers them.
//! Rendered views are made off the UI thread where threads exist.
//!
//! Control. Reading calls always run, even while changes queued before them
//! wait. A changing call (or one that changes the selection) runs at once unless
//! the designer is in the middle of an operation (drawing, pushing, typing a
//! value, …) or has taken control; then BenchUp asks them to grant or deny
//! access. Granting cancels their operation and lets the agent continue;
//! denying refuses the waiting calls. While the agent is editing, the status
//! bar offers "Take control", after which its changes wait for a grant again.
use super::*;
use benchup_tools::{Call, ToolEnv, batch, pairing};
use serde_json::{Value, json};
use std::collections::VecDeque;

#[path = "agent_link_ui.rs"]
mod ui;
#[cfg(test)]
#[path = "agent_link_tests.rs"]
mod tests;

pub(super) const DEFAULT_PORT: u16 = pairing::DEFAULT_PORT;
const BACKOFF_START: f64 = 1.;
const BACKOFF_MAX: f64 = 10.;
/// The bridge must answer the handshake within this long.
const READY_TIMEOUT: f64 = 15.;
/// The bridge gives up on a call after 120 s; answer first.
const QUEUE_TIMEOUT: f64 = 100.;
const QUEUE_LIMIT: usize = 64;
/// Calls run per frame, so a burst cannot freeze the UI.
const CALLS_PER_FRAME: usize = 8;
const MAX_CLIENT_CHARS: usize = pairing::MAX_CLIENT_CHARS;
/// The largest message accepted from the bridge (the browser build allows
/// about the same); the bridge refuses larger calls itself.
const MAX_MESSAGE_BYTES: usize = pairing::MAX_APP_MESSAGE_BYTES;
/// The largest result sent: the bridge drops the connection on anything
/// bigger, so a larger answer is replaced with an error.
const MAX_RESULT_BYTES: usize = pairing::MAX_BRIDGE_MESSAGE_BYTES;
/// A call is not run in the last second before its deadline.
const DEADLINE_MARGIN_MS: u64 = 1000;
/// How long to wait before trying again to make or save a token.
const TOKEN_RETRY: f64 = 30.;
/// Granted control returns to normal after the agent is quiet this long.
const AGENT_IDLE: f64 = 8.;
const TOKEN_FILE: &str = "agent-link.token";
const TOKEN_STORAGE: &str = "benchup.agent_link.token";

/// Remembered across restarts (in `GeneralSettings`). The pairing token is
/// deliberately not here: it stays in this device's private storage.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Preferences {
    pub enabled: bool,
    pub port: u16,
}

impl Default for Preferences {
    fn default() -> Self {
        Self { enabled: false, port: DEFAULT_PORT }
    }
}

impl Preferences {
    pub(super) fn normalize(&mut self) {
        if self.port < 1024 {
            self.port = DEFAULT_PORT;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Status {
    Off,
    Connecting,
    Connected { client: Option<String> },
    Error(String),
}

/// Who may change the model right now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Control {
    /// The agent edits while the designer is idle; otherwise it asks.
    #[default]
    Shared,
    /// The designer granted access; the agent edits until it goes quiet.
    Agent,
    /// The designer took control; every change waits for a grant.
    Designer,
}

/// One live connection, abstracted so tests need no bridge.
pub(super) trait Link {
    /// Everything received since the last call. An `Err` ends the connection.
    fn events(&mut self) -> Vec<Result<Value, String>>;
    fn send(&mut self, message: Value) -> Result<(), String>;
}
pub(super) trait Connector {
    /// Starts connecting; the socket sends the `authenticate` hello itself.
    fn connect(&mut self, ctx: &egui::Context, url: &str, connection: &str) -> Box<dyn Link>;
}

struct SocketLink(crate::realtime::Socket);
impl Link for SocketLink {
    fn events(&mut self) -> Vec<Result<Value, String>> {
        use std::sync::mpsc::TryRecvError;
        let mut events = Vec::new();
        loop {
            match self.0.events.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    events.push(Err(tr!("realtime.websocket_disconnected").into()));
                    break;
                }
            }
        }
        events
    }
    fn send(&mut self, message: Value) -> Result<(), String> {
        self.0.send(message)
    }
}
struct SocketConnector;
impl Connector for SocketConnector {
    fn connect(&mut self, ctx: &egui::Context, url: &str, connection: &str) -> Box<dyn Link> {
        // The ticket stays empty: the token is proved, never sent.
        Box::new(SocketLink(crate::realtime::Socket::connect_limited(
            ctx,
            url.into(),
            String::new(),
            connection.into(),
            Some(MAX_MESSAGE_BYTES),
        )))
    }
}

/// What the bridge can send.
#[derive(Debug, PartialEq)]
pub(super) enum Frame {
    Challenge { nonce: String, protocol: Option<u64> },
    Ready { proof: Option<String>, protocol: Option<u64> },
    /// A refusal: its `code` (translated when known) and English text.
    Error { code: Option<String>, message: String },
    Cancel { id: String },
    Call { id: String, client: String, deadline_ms: Option<u64>, call: Result<(String, Value), String> },
    /// Unknown type, or not an object: ignored.
    Ignored,
}

/// Characters shown in the UI: not control or invisible formatting
/// characters (bidi overrides, zero-width characters, tag characters).
fn printable(c: char) -> bool {
    !c.is_control()
        && !matches!(c as u32,
            0xAD | 0x600..=0x605 | 0x61C | 0x6DD | 0x70F | 0x180E
            | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F
            | 0xFEFF | 0xFFF9..=0xFFFB | 0xE0001 | 0xE0020..=0xE007F)
}
pub(super) fn clean_client(client: &str) -> String {
    let clean: String = client.chars().filter(|c| printable(*c)).collect();
    clean.trim().chars().take(MAX_CLIENT_CHARS).collect::<String>().trim().to_owned()
}

/// Seconds for the connection's timers. Not `egui::InputState::time`, which
/// stands still while the window is minimized or the tab is hidden.
pub(super) fn clock() -> f64 {
    now_ms() as f64 / 1000.
}

/// The length of `value` as JSON text, without building the text.
fn json_len(value: &Value) -> usize {
    struct Count(usize);
    impl std::io::Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    let _ = serde_json::to_writer(&mut count, value);
    count.0
}

/// Unix time in milliseconds (the unit of a call's `deadline_ms`).
fn now_ms() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
}

/// 32 random bytes from the OS (native) or Web Crypto (browser) as 64
/// lowercase hex characters, or an error if there is no secure randomness.
fn new_nonce() -> Result<String, String> {
    std::panic::catch_unwind(|| {
        let (a, b) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let bytes: Vec<u8> = a.as_bytes().iter().chain(b.as_bytes()).copied().collect();
        pairing::to_hex(&bytes)
    })
    .map_err(|_| tr!("agent_link.error.no_randomness").to_owned())
}

/// A new random version-4 UUID. If no secure randomness is available no
/// token is made: it must never be guessable.
pub(super) fn new_token() -> Result<String, String> {
    // `uuid` panics rather than return weak bytes when the system RNG fails.
    std::panic::catch_unwind(|| uuid::Uuid::new_v4().to_string())
        .map_err(|_| tr!("agent_link.error.no_randomness").to_owned())
}

/// Strict frame parsing: anything unexpected is `Ignored`, except a call that
/// has an id, which is always answered (with an error if it is malformed).
pub(super) fn parse_frame(value: &Value) -> Frame {
    let Some(object) = value.as_object() else { return Frame::Ignored };
    match object.get("type").and_then(Value::as_str) {
        Some("challenge") => Frame::Challenge {
            nonce: object.get("nonce").and_then(Value::as_str).unwrap_or_default().to_owned(),
            protocol: object.get("protocol").and_then(Value::as_u64),
        },
        Some("ready") => Frame::Ready {
            proof: object.get("proof").and_then(Value::as_str).map(str::to_owned),
            protocol: object.get("protocol").and_then(Value::as_u64),
        },
        Some("cancel") => match object.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) {
            Some(id) => Frame::Cancel { id: id.to_owned() },
            None => Frame::Ignored,
        },
        Some("error") => Frame::Error {
            code: object.get("code").and_then(Value::as_str).map(str::to_owned),
            message: object
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .filter(|c| printable(*c))
                .take(200)
                .collect(),
        },
        Some("call") => {
            let Some(id) = object.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()) else {
                return Frame::Ignored;
            };
            let client = object.get("client").and_then(Value::as_str).map(clean_client).unwrap_or_default();
            let call = (|| {
                let name = object
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| tr!("agent_link.error.missing_tool").to_owned())?;
                let arguments = match object.get("arguments") {
                    None | Some(Value::Null) => json!({}),
                    Some(arguments) if arguments.is_object() => arguments.clone(),
                    Some(_) => return Err(tr!("agent_link.error.arguments_not_object").to_owned()),
                };
                Ok((name.to_owned(), arguments))
            })();
            let deadline_ms = object.get("deadline_ms").and_then(|v| {
                v.as_u64().or_else(|| v.as_f64().filter(|f| f.is_finite() && *f >= 0.).map(|f| f as u64))
            });
            Frame::Call { id: id.to_owned(), client, deadline_ms, call }
        }
        _ => Frame::Ignored,
    }
}

/// Whether a call can change the model, so it may need the designer's grant.
pub(super) fn changes_model(name: &str, arguments: &Value) -> bool {
    match name {
        "undo" => true,
        "render_view" => false,
        "run_batch" => {
            !arguments.get("dry_run").and_then(Value::as_bool).unwrap_or(false)
                && arguments.get("calls").and_then(Value::as_array).is_some_and(|calls| {
                    calls.iter().any(|call| call.get("tool").and_then(Value::as_str).is_some_and(benchup_tools::is_mutating))
                })
        }
        name => benchup_tools::is_mutating(name),
    }
}

/// Whether a call replaces the designer's selection (the `select` tool).
pub(super) fn changes_selection(name: &str, arguments: &Value) -> bool {
    match name {
        "select" => true,
        "run_batch" => {
            !arguments.get("dry_run").and_then(Value::as_bool).unwrap_or(false)
                && arguments.get("calls").and_then(Value::as_array).is_some_and(|calls| {
                    calls.iter().any(|call| call.get("tool").and_then(Value::as_str) == Some("select"))
                })
        }
        _ => false,
    }
}

/// Whether a call must wait for the designer when they are busy or have
/// taken control: it changes the model, or swaps the selection an operation
/// in progress works on.
pub(super) fn needs_grant(name: &str, arguments: &Value) -> bool {
    changes_model(name, arguments) || changes_selection(name, arguments)
}

struct Queued {
    id: String,
    client: String,
    name: String,
    arguments: Value,
    /// Changes the model or the selection, so it may need a grant.
    changes: bool,
    queued_at: f64,
    /// The bridge was told the call is waiting.
    notified: bool,
    /// Unix ms after which the bridge no longer wants an answer.
    deadline_ms: u64,
    /// The document epoch when the call arrived.
    epoch: u64,
}

/// Where the pairing handshake is.
#[derive(Clone, Debug, PartialEq)]
enum Phase {
    /// Waiting for the bridge's `challenge`.
    Challenge,
    /// `prove` sent; waiting for a `ready` carrying the bridge's proof.
    Proved { bridge_nonce: String, app_nonce: String },
    /// The bridge proved the token. Only now are calls accepted.
    Ready,
}

struct Active {
    link: Box<dyn Link>,
    port: u16,
    token: String,
    started: f64,
    phase: Phase,
}

pub(super) struct AgentLink {
    pub status: Status,
    /// The pairing token, loaded from this device on demand.
    pub token: Option<String>,
    pub token_error: Option<String>,
    token_checked: bool,
    /// After a failure, no new token is tried before this time.
    token_retry_at: f64,
    /// Makes tokens; replaced in tests to simulate a missing RNG.
    generator: fn() -> Result<String, String>,
    /// Session-only: revealing a secret is never remembered.
    pub show_token: bool,
    active: Option<Active>,
    connector: Box<dyn Connector>,
    queue: VecDeque<Queued>,
    retry_at: f64,
    backoff: f64,
    /// The open document, refreshed by the app every frame: replies carry it.
    document_name: String,
    epoch: u64,
    /// Makes the app's handshake nonce; replaced in tests.
    nonce_source: fn() -> Result<String, String>,
    pub control: Control,
    /// The grant/deny dialog is waiting for the designer.
    pub asking: bool,
    /// The designer denied access; further changes are refused without
    /// asking until they finish what they are doing.
    denied: bool,
    /// When the agent last changed the model.
    last_change: Option<f64>,
    /// The last copy button pressed and until when "Copied" shows (session only).
    copied: Option<(ui::CopiedText, f64)>,
    /// The `render_view` being made on another thread: its call id, and
    /// where the answer arrives.
    #[cfg(not(target_arch = "wasm32"))]
    rendering: Option<(String, std::sync::mpsc::Receiver<(bool, Value)>)>,
    /// Results larger than this are refused (lowered in tests).
    max_result_bytes: usize,
    /// Token storage; tests use a temporary slot.
    #[cfg(test)]
    token_slot: Option<std::path::PathBuf>,
}

impl Default for AgentLink {
    fn default() -> Self {
        Self::with_connector(Box::new(SocketConnector))
    }
}

/// `claude mcp add` command for this token (and port, when not the default).
/// The token goes in the environment, never in the program's arguments (which
/// other local users can read).
pub(super) fn claude_command(token: &str, port: u16) -> String {
    let port = if port == DEFAULT_PORT { String::new() } else { format!(" --port {port}") };
    format!("claude mcp add benchup -e BENCHUP_AGENT_TOKEN={token} -- benchup bridge{port}")
}

/// Generic `mcpServers` entry; the token travels in the environment.
pub(super) fn mcp_json(token: &str, port: u16) -> String {
    let mut args = vec![json!("bridge")];
    if port != DEFAULT_PORT {
        args.extend([json!("--port"), json!(port.to_string())]);
    }
    serde_json::to_string_pretty(&json!({
        "mcpServers": {"benchup": {"command": "benchup", "args": args, "env": {"BENCHUP_AGENT_TOKEN": token}}}
    }))
    .unwrap_or_default()
}

/// Shell lines for agents that use the `benchup` command (`--live`).
pub(super) fn shell_setup(token: &str, port: u16) -> String {
    let port = if port == DEFAULT_PORT { String::new() } else { format!("\nexport BENCHUP_BRIDGE_PORT={port}") };
    format!("export BENCHUP_AGENT_TOKEN={token}{port}\nbenchup scene --live")
}

impl AgentLink {
    pub(super) fn with_connector(connector: Box<dyn Connector>) -> Self {
        Self {
            status: Status::Off,
            token: None,
            token_error: None,
            token_checked: false,
            token_retry_at: 0.,
            generator: new_token,
            show_token: false,
            active: None,
            connector,
            queue: VecDeque::new(),
            retry_at: 0.,
            backoff: BACKOFF_START,
            document_name: String::new(),
            epoch: 0,
            nonce_source: new_nonce,
            control: Control::Shared,
            asking: false,
            denied: false,
            last_change: None,
            copied: None,
            #[cfg(not(target_arch = "wasm32"))]
            rendering: None,
            max_result_bytes: MAX_RESULT_BYTES,
            #[cfg(test)]
            token_slot: None,
        }
    }

    fn stored_token(&self) -> Option<String> {
        #[cfg(test)]
        if let Some(dir) = &self.token_slot {
            return super::credentials::load_in(dir, TOKEN_FILE);
        }
        super::credentials::load(TOKEN_FILE, TOKEN_STORAGE)
    }
    fn store_token(&self, token: &str) -> Result<(), String> {
        #[cfg(test)]
        if let Some(dir) = &self.token_slot {
            return super::credentials::save_in(dir, TOKEN_FILE, token);
        }
        super::credentials::save(TOKEN_FILE, TOKEN_STORAGE, token)
    }

    /// Reads the saved token, without creating one (safe to call from the UI).
    pub(super) fn load_token(&mut self) {
        if !self.token_checked {
            self.token_checked = true;
            self.token = self.stored_token();
        }
    }
    /// The saved token, or a new one saved now; `None` if none could be made
    /// (the reason is in `token_error`). After a failure it waits
    /// `TOKEN_RETRY` before trying again, rather than write every frame.
    fn ensure_token(&mut self, now: f64) -> Option<String> {
        self.load_token();
        if self.token.is_none() && now >= self.token_retry_at {
            self.regenerate();
            if self.token.is_none() {
                self.token_retry_at = now + TOKEN_RETRY;
            }
        }
        self.token.clone()
    }
    /// Replaces the token. Connections made with the old one are dropped. If
    /// a new token cannot be made or saved, the old one stays (an unsaved
    /// token would stop pairing after a restart) and `token_error` says why.
    pub(super) fn regenerate(&mut self) {
        let made = (self.generator)().and_then(|token| self.store_token(&token).map(|()| token));
        match made {
            Ok(token) => {
                self.token_error = None;
                self.token_checked = true;
                self.token = Some(token);
            }
            Err(error) => self.token_error = Some(error),
        }
    }

    pub(super) fn connected(&self) -> bool {
        matches!(self.status, Status::Connected { .. })
    }
    /// The agent changed the model recently, or is waiting to.
    pub(super) fn editing(&self, now: f64) -> bool {
        self.control == Control::Agent
            || self.queue.iter().any(|queued| queued.changes)
            || self.last_change.is_some_and(|at| now - at < AGENT_IDLE)
    }
    /// Who is asking, for the dialog.
    pub(super) fn waiting_client(&self) -> Option<&str> {
        self.queue.iter().find(|queued| queued.changes).map(|queued| queued.client.as_str())
    }
    pub(super) fn waiting_changes(&self) -> usize {
        self.queue.iter().filter(|queued| queued.changes).count()
    }
    pub(super) fn client_name(&self) -> Option<&str> {
        match &self.status {
            Status::Connected { client: Some(client) } => Some(client),
            _ => None,
        }
    }

    fn send(&mut self, message: Value) {
        if let Some(active) = &mut self.active {
            let _ = active.link.send(message);
        }
    }
    /// A `result` frame; every result object says which project was open, so
    /// an agent can notice a switch.
    fn result_frame(&self, id: &str, ok: bool, mut result: Value) -> Value {
        if let Some(object) = result.as_object_mut() {
            object.insert("document".into(), json!({"name": self.document_name}));
        }
        json!({"type": "result", "id": id, "ok": ok, "result": result})
    }
    /// Sends a result, or an error if it is too large for the bridge.
    /// `applied`: the call changed the model, which stays changed.
    fn send_result(&mut self, id: &str, ok: bool, result: Value, applied: bool) {
        let mut frame = self.result_frame(id, ok, result);
        if json_len(&frame) > self.max_result_bytes {
            let mut refusal = json!({"ok": false,
                "error": tr!("agent_link.error.result_too_large", size = self.max_result_bytes / (1024 * 1024)),
                "hint": tr!("agent_link.error.result_too_large_hint")});
            if applied {
                refusal["applied"] = json!(true);
            }
            frame = self.result_frame(id, false, refusal);
        }
        self.send(frame);
    }
    fn refuse(&mut self, id: &str, error: &str, hint: Option<&str>) {
        let mut result = json!({"ok": false, "error": error});
        if let Some(hint) = hint {
            result["hint"] = json!(hint);
        }
        self.send_result(id, false, result, false);
    }
    /// Refuses every waiting call that would change the model.
    fn refuse_changes(&mut self, error: &str, hint: Option<&str>) {
        let (refused, kept): (VecDeque<Queued>, VecDeque<Queued>) =
            std::mem::take(&mut self.queue).into_iter().partition(|queued| queued.changes);
        self.queue = kept;
        for queued in refused {
            self.refuse(&queued.id, error, hint);
        }
        self.asking = false;
    }

    /// The designer takes control: waiting changes are refused and later ones
    /// wait for a grant.
    pub(super) fn take_control(&mut self) {
        self.control = Control::Designer;
        self.last_change = None;
        self.refuse_changes(tr!("agent_link.error.taken_control"), Some(tr!("agent_link.error.denied_hint")));
    }
    /// The designer lets the agent edit again without asking.
    pub(super) fn release_control(&mut self) {
        self.control = Control::Shared;
        self.denied = false;
    }
    /// The designer refused the waiting changes.
    pub(super) fn deny(&mut self) {
        self.denied = true;
        self.refuse_changes(tr!("agent_link.error.denied"), Some(tr!("agent_link.error.denied_hint")));
    }

    /// Drops the connection; queued calls can no longer be answered.
    fn stop(&mut self) {
        self.active = None;
        self.queue.clear();
        self.asking = false;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.rendering = None;
        }
    }
    fn fail(&mut self, now: f64, message: String) {
        self.stop();
        self.status = Status::Error(message);
        self.retry_at = now + self.backoff;
        self.backoff = (self.backoff * 2.).min(BACKOFF_MAX);
    }

    /// Keeps the connection in the state the preferences ask for, and
    /// collects incoming calls into the queue. Never blocks.
    pub(super) fn pump(&mut self, ctx: &egui::Context, now: f64, prefs: &Preferences) {
        if !prefs.enabled {
            if let Some(mut active) = self.active.take() {
                // Answer what is waiting, then close.
                for queued in std::mem::take(&mut self.queue) {
                    let reply = self.result_frame(&queued.id, false, json!({"ok": false, "error": tr!("agent_link.error.disabled")}));
                    let _ = active.link.send(reply);
                }
            }
            self.stop();
            self.status = Status::Off;
            self.control = Control::Shared;
            self.backoff = BACKOFF_START;
            self.retry_at = 0.;
            return;
        }
        let Some(token) = self.ensure_token(now) else {
            // No secure token, so nothing connects.
            self.stop();
            self.status = Status::Error(self.token_error.clone().unwrap_or_default());
            return;
        };
        if self.active.as_ref().is_some_and(|a| a.port != prefs.port || a.token != token) {
            self.stop();
            self.backoff = BACKOFF_START;
            self.retry_at = 0.;
        }
        if self.active.is_none() {
            if now < self.retry_at {
                ctx.request_repaint_after(Duration::from_secs_f64(self.retry_at - now));
                return;
            }
            let url = format!("ws://127.0.0.1:{}", prefs.port);
            let link = self.connector.connect(ctx, &url, concat!("benchup-app-", env!("CARGO_PKG_VERSION")));
            self.active = Some(Active { link, port: prefs.port, token: token.clone(), started: now, phase: Phase::Challenge });
            self.status = Status::Connecting;
        }
        let Some(active) = &mut self.active else { return };
        let events = active.link.events();
        let started = active.started;
        for event in events {
            let value = match event {
                Ok(value) => value,
                Err(message) => return self.fail(now, message),
            };
            let Some(phase) = self.active.as_ref().map(|a| a.phase.clone()) else { return };
            match parse_frame(&value) {
                Frame::Challenge { nonce, protocol } => {
                    // Exactly one challenge, first.
                    if phase != Phase::Challenge || !pairing::valid_nonce(&nonce) {
                        return self.fail(now, tr!("agent_link.error.handshake").into());
                    }
                    if protocol != Some(u64::from(pairing::PROTOCOL)) {
                        return self.fail(now, tr!("agent_link.error.protocol").into());
                    }
                    let app_nonce = match (self.nonce_source)() {
                        Ok(nonce) => nonce,
                        Err(message) => return self.fail(now, message),
                    };
                    let proof = pairing::app_proof(&token, &nonce, &app_nonce);
                    self.send(json!({"type": "prove", "nonce": app_nonce, "proof": proof}));
                    if let Some(active) = &mut self.active {
                        active.phase = Phase::Proved { bridge_nonce: nonce, app_nonce };
                    }
                }
                Frame::Ready { proof, protocol } => match phase {
                    Phase::Challenge => return self.fail(now, tr!("agent_link.error.protocol").into()),
                    Phase::Proved { bridge_nonce, app_nonce } => {
                        if protocol.is_some_and(|p| p != u64::from(pairing::PROTOCOL)) {
                            return self.fail(now, tr!("agent_link.error.protocol").into());
                        }
                        let expected = pairing::bridge_proof(&token, &bridge_nonce, &app_nonce);
                        if !proof.as_deref().is_some_and(|proof| pairing::proofs_match(proof, &expected)) {
                            return self.fail(now, tr!("agent_link.error.unproven").into());
                        }
                        if let Some(active) = &mut self.active {
                            active.phase = Phase::Ready;
                        }
                        self.status = Status::Connected { client: None };
                        self.backoff = BACKOFF_START;
                    }
                    Phase::Ready => {}
                },
                Frame::Error { code, message } => {
                    let reason = match code.as_deref() {
                        Some(pairing::refusal::APP_BUSY) => tr!("agent_link.error.app_busy").to_owned(),
                        Some(pairing::refusal::TOKEN_MISMATCH) => tr!("agent_link.error.unproven").to_owned(),
                        Some(pairing::refusal::UNKNOWN_ROLE | pairing::refusal::UNEXPECTED) => {
                            tr!("agent_link.error.protocol").to_owned()
                        }
                        // An older bridge, or a reason this app does not know.
                        _ => tr!("agent_link.error.bridge", message = message),
                    };
                    return self.fail(now, reason);
                }
                Frame::Cancel { id } => {
                    if phase == Phase::Ready {
                        self.queue.retain(|queued| queued.id != id);
                    }
                }
                Frame::Call { id, client, deadline_ms, call } => {
                    if phase != Phase::Ready {
                        // Calls before a verified `ready` are not trusted.
                        continue;
                    }
                    // No deadline: the bridge's own limit, as a cap.
                    let deadline_ms = deadline_ms.unwrap_or(now_ms() + (QUEUE_TIMEOUT * 1000.) as u64);
                    match call {
                        Err(message) => self.refuse(&id, &message, None),
                        Ok(_) if self.queue.len() >= QUEUE_LIMIT => self.refuse(&id, tr!("agent_link.error.queue_full"), None),
                        Ok((name, arguments)) => {
                            if !client.is_empty() {
                                self.status = Status::Connected { client: Some(client.clone()) };
                            }
                            let changes = needs_grant(&name, &arguments);
                            let epoch = self.epoch;
                            self.queue.push_back(Queued {
                                id,
                                client,
                                name,
                                arguments,
                                changes,
                                queued_at: now,
                                notified: false,
                                deadline_ms,
                                epoch,
                            });
                        }
                    }
                }
                Frame::Ignored => {}
            }
        }
        if self.status == Status::Connecting {
            if now - started > READY_TIMEOUT {
                self.fail(now, tr!("agent_link.error.connect_timeout").into());
            } else {
                ctx.request_repaint_after(Duration::from_secs_f64((READY_TIMEOUT - (now - started)).max(0.1)));
            }
        }
    }

    /// Drops calls past their deadline and answers those that waited longer
    /// than the bridge will. The only place deadlines are checked.
    fn expire(&mut self, now: f64) {
        // The bridge has already answered calls that are about to pass their
        // deadline; running them would only change the model unseen.
        let limit = now_ms() + DEADLINE_MARGIN_MS;
        self.queue.retain(|queued| queued.deadline_ms > limit);
        let (stale, kept): (VecDeque<Queued>, VecDeque<Queued>) =
            std::mem::take(&mut self.queue).into_iter().partition(|queued| now - queued.queued_at > QUEUE_TIMEOUT);
        self.queue = kept;
        for queued in stale {
            self.refuse(&queued.id, tr!("agent_link.error.busy"), None);
        }
    }
    /// Tells the bridge once that a call is waiting.
    fn notify_waiting(&mut self, message: &str) {
        let mut fresh = false;
        for queued in self.queue.iter_mut().filter(|q| !q.notified) {
            queued.notified = true;
            fresh = true;
        }
        if fresh {
            self.send(json!({"type": "status", "message": message}));
        }
    }
}

/// Changing tools named in a call, for the history label.
fn summary(name: &str, arguments: &Value) -> String {
    if name != "run_batch" {
        return name.to_owned();
    }
    let mut tools: Vec<&str> = Vec::new();
    for call in arguments.get("calls").and_then(Value::as_array).into_iter().flatten() {
        if let Some(tool) = call.get("tool").and_then(Value::as_str).filter(|tool| benchup_tools::is_mutating(tool))
            && !tools.contains(&tool)
        {
            tools.push(tool);
        }
    }
    let more = tools.len() > 3;
    tools.truncate(3);
    let mut text = tools.join(", ");
    if more {
        text.push_str(", …");
    }
    text
}

impl BenchUpApp {
    /// Called from `App::logic` every frame, also while the window is
    /// minimized or the tab hidden: keeps the connection up and runs waiting
    /// calls.
    pub(super) fn poll_agent_link(&mut self, ctx: &egui::Context) {
        let now = clock();
        let prefs = self.general_settings.agent.clone();
        self.agent_link.epoch = self.doc_epoch;
        self.agent_link.document_name = self.doc.name.clone();
        self.agent_link.pump(ctx, now, &prefs);
        self.agent_link.expire(now);
        self.finish_agent_render();
        let busy = self.user_operation_active();
        let link = &mut self.agent_link;
        if link.control == Control::Agent
            && !link.queue.iter().any(|q| q.changes)
            && link.last_change.is_none_or(|at| now - at >= AGENT_IDLE)
        {
            link.control = Control::Shared;
        }
        if link.denied && !busy && link.control != Control::Designer {
            link.denied = false;
        }
        let mut ran = 0;
        let mut waiting = None;
        // On the web a render runs here, so at most one per frame.
        #[cfg(target_arch = "wasm32")]
        let mut rendered = false;
        // Calls that must wait stay queued, in order; the calls behind them
        // still run if they can (reads always can).
        let mut index = 0;
        while index < self.agent_link.queue.len() {
            // A file operation is replacing the model: wait for it.
            if self.project.blocked() {
                waiting = Some(tr!("agent_link.queued_status"));
                break;
            }
            if ran >= CALLS_PER_FRAME {
                ctx.request_repaint();
                break;
            }
            let queued = &self.agent_link.queue[index];
            let (changes, stale, render) =
                (queued.changes, queued.epoch != self.doc_epoch, queued.name == "render_view");
            let edits = changes && changes_model(&queued.name, &queued.arguments);
            if stale {
                if let Some(queued) = self.agent_link.queue.remove(index) {
                    self.agent_link.refuse(
                        &queued.id,
                        tr!("agent_link.error.document_changed"),
                        Some(tr!("agent_link.error.document_changed_hint")),
                    );
                }
                continue;
            }
            if changes {
                let ask = self.agent_must_ask();
                if ask && self.agent_link.denied {
                    if let Some(queued) = self.agent_link.queue.remove(index) {
                        self.agent_link.refuse(&queued.id, tr!("agent_link.error.denied"), Some(tr!("agent_link.error.denied_hint")));
                    }
                    continue;
                }
                if ask {
                    waiting = Some(tr!("agent_link.waiting_for_designer"));
                    index += 1;
                    continue;
                }
                // A shared project's selection lease is on its way to the
                // server: the edit waits for it rather than fail.
                if edits && self.shared_lease_pending(ctx) {
                    waiting = Some(tr!("agent_link.waiting_for_server"));
                    index += 1;
                    continue;
                }
                if self.user_operation_active() {
                    // Granted control: the designer's new operation gives way.
                    self.cancel_user_operation(ctx);
                }
            }
            if render {
                #[cfg(not(target_arch = "wasm32"))]
                if self.agent_link.rendering.is_some() {
                    index += 1;
                    continue;
                }
                #[cfg(target_arch = "wasm32")]
                if rendered {
                    ctx.request_repaint();
                    index += 1;
                    continue;
                }
            }
            let Some(queued) = self.agent_link.queue.remove(index) else { break };
            ran += 1;
            if render {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.start_agent_render(ctx, queued.id, queued.arguments);
                    continue;
                }
                #[cfg(target_arch = "wasm32")]
                {
                    rendered = true;
                }
            }
            let selection_before = self.selected.clone();
            let (ok, result) = self.run_agent_request(&queued.name, &queued.arguments, ctx);
            // `select` or `undo` may change the selection; in a shared
            // project the server must lease it before edits can follow.
            self.selection_changed_outside_frame(&selection_before);
            let applied = ok && changes_model(&queued.name, &queued.arguments);
            if applied {
                self.agent_link.last_change = Some(now);
            }
            self.agent_link.document_name = self.doc.name.clone();
            self.agent_link.send_result(&queued.id, ok, result, applied);
        }
        // Worked out afresh every frame: the dialog shows only while a waiting
        // change actually needs the designer's grant (it goes away as soon as
        // they finish or cancel their operation, even with calls still queued).
        let must_ask = self.agent_must_ask();
        let link = &mut self.agent_link;
        link.asking = must_ask && !link.denied && link.queue.iter().any(|q| q.changes);
        if let Some(message) = waiting {
            self.agent_link.notify_waiting(message);
            // Deadlines, leases and file operations end without input events.
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if self.agent_link.editing(now) {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }

    /// Renders a view on another thread from a copy of the model, so a large
    /// render does not freeze the window; `finish_agent_render` answers.
    #[cfg(not(target_arch = "wasm32"))]
    fn start_agent_render(&mut self, ctx: &egui::Context, id: String, arguments: Value) {
        let model = self.doc.model.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(render_answer(&model, &arguments));
            ctx.request_repaint();
        });
        self.agent_link.rendering = Some((id, rx));
    }

    /// Answers the render made on another thread, once it is done.
    fn finish_agent_render(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::mpsc::TryRecvError;
            let Some((id, rx)) = &self.agent_link.rendering else { return };
            let answer = match rx.try_recv() {
                Ok(answer) => answer,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => (false, json!({"ok": false, "error": "the render failed"})),
            };
            let id = id.clone();
            self.agent_link.rendering = None;
            self.agent_link.document_name = self.doc.name.clone();
            self.agent_link.send_result(&id, answer.0, answer.1, false);
        }
    }

    /// A waiting change needs the designer's grant before it runs.
    fn agent_must_ask(&self) -> bool {
        match self.agent_link.control {
            Control::Designer => true,
            Control::Shared => self.user_operation_active(),
            Control::Agent => false,
        }
    }

    /// The designer is in the middle of something an agent edit would break.
    pub(super) fn user_operation_active(&self) -> bool {
        self.drawing.is_some()
            || self.modeling.is_some()
            || self.line_start.is_some()
            || self.grain_anchor.is_some()
            || self.paste_preview.is_some()
            // A finished Move-copy only offers an array until the next edit;
            // it is an operation once the designer starts typing the count.
            || (self.last_move_copy.is_some() && !self.length_input.trim().is_empty())
            || self.solid_first.is_some()
            || self.measurement.is_some()
            || self.text_editor.is_some()
            || self.object_creation.is_some()
            || self.object_rename.is_some()
            || self.gesture.as_ref().is_some_and(|gesture| {
                !matches!(
                    gesture.kind,
                    GestureKind::Navigate { .. } | GestureKind::DrawingNavigate | GestureKind::Compass { .. }
                )
            })
    }

    /// Ends the designer's current operation without applying it, like Escape.
    pub(super) fn cancel_user_operation(&mut self, ctx: &egui::Context) {
        self.stop_drawing();
        self.modeling = None;
        self.measurement = None;
        self.solid_first = None;
        self.last_move_copy = None;
        self.paste_preview = None;
        self.text_editor = None;
        self.object_creation = None;
        self.object_rename = None;
        self.flip_hover = None;
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        ctx.request_repaint();
    }

    /// The designer granted access from the dialog.
    pub(super) fn grant_agent_access(&mut self, ctx: &egui::Context) {
        self.cancel_user_operation(ctx);
        let link = &mut self.agent_link;
        link.control = Control::Agent;
        link.denied = false;
        link.asking = false;
        link.last_change = Some(clock());
        ctx.request_repaint();
    }

    /// The tool environment for the open document: purchasing catalog,
    /// read-only and lease rules for shared projects, and the selection.
    pub(super) fn tool_env(&self) -> ToolEnv {
        let mut catalog: std::collections::BTreeMap<_, _> = self.doc.model.stock_materials.clone();
        catalog.extend(self.general_settings.stock.wood.iter().map(|stock| (stock.id, stock.definition())));
        ToolEnv {
            // Only a view-only share is read-only. Other refusals (no
            // connection, a lease on its way) are reported when the edit is
            // recorded, with the actual reason.
            read_only: self.shared_view_only(),
            // Agents see the current selection: a designer can say "change
            // this" and the agent finds what "this" is.
            selection: self.selected.clone(),
            hide_selection: false,
            catalog,
            before: None,
            lease: self.shared_lease(),
            created: BTreeSet::new(),
        }
    }

    /// Runs one request from the bridge: a tool, `run_batch`, `undo` or
    /// `render_view`.
    pub(super) fn run_agent_request(&mut self, name: &str, arguments: &Value, ctx: &egui::Context) -> (bool, Value) {
        match name {
            "undo" => self.agent_undo(arguments.get("redo").and_then(Value::as_bool).unwrap_or(false), ctx),
            "render_view" => render_answer(&self.doc.model, arguments),
            "run_batch" => {
                let Some(list) = arguments.get("calls").and_then(Value::as_array).filter(|list| !list.is_empty()) else {
                    return (false, json!({"ok": false, "error": tr!("agent_link.error.calls_list")}));
                };
                let mut calls = Vec::with_capacity(list.len());
                for call in list {
                    let Some(tool) = call.get("tool").and_then(Value::as_str) else {
                        return (false, json!({"ok": false, "error": tr!("agent_link.error.calls_list")}));
                    };
                    calls.push((tool.to_owned(), call.get("arguments").cloned().unwrap_or_else(|| json!({}))));
                }
                let dry_run = arguments.get("dry_run").and_then(Value::as_bool).unwrap_or(false);
                // `single` (`benchup call --live --dry-run`): one call keeps
                // "$" text as written and answers with its own result.
                let single = calls.len() == 1 && arguments.get("single").and_then(Value::as_bool).unwrap_or(false);
                let label = tr!("agent_link.history_label", summary = summary(name, arguments));
                let (mut value, ok) = self.run_agent_calls(&calls, !single, dry_run, &label, ctx);
                if dry_run && let Some(object) = value.as_object_mut() {
                    object.insert("dry_run".into(), json!(true));
                }
                (ok, value)
            }
            _ => {
                let label = tr!("agent_link.history_label", summary = summary(name, arguments));
                let (value, ok) = self.run_agent_calls(&[(name.to_owned(), arguments.clone())], false, false, &label, ctx);
                (ok, value)
            }
        }
    }

    /// Runs calls in order as one all-or-nothing step: if any fails, the model
    /// is left as it was. Changes are committed as one history step, like the
    /// designer's own edits; a dry run reports the results and changes nothing.
    pub(super) fn run_agent_calls(
        &mut self,
        calls: &[(String, Value)],
        batch: bool,
        dry_run: bool,
        label: &str,
        ctx: &egui::Context,
    ) -> (Value, bool) {
        let mut env = self.tool_env();
        let scope = self.doc.model.edit_context;
        let mut results: Vec<Value> = Vec::with_capacity(calls.len());
        let (mut created, mut modified) = (Vec::new(), Vec::new());
        for (index, (name, arguments)) in calls.iter().enumerate() {
            let failure = |result: Value, earlier: Vec<Value>| batch::Applied::Failed { index, result, earlier };
            let arguments = if batch { batch::resolve(arguments, &results) } else { Ok(arguments.clone()) };
            let outcome = match arguments {
                Err(error) => Err(json!({"ok": false, "error": error,
                    "hint": "Refer to an earlier call as \"$N.field\" (N counts calls from 1), e.g. \"$1.created[0]\"."})),
                Ok(_) if matches!(name.as_str(), "run_batch" | "undo" | "render_view") => {
                    Err(json!({"ok": false, "error": tr!("agent_link.error.nested_session", tool = name)}))
                }
                Ok(arguments) => {
                    let outcome = benchup_tools::execute(&mut self.doc.model, &Call::new(format!("agent-{index}"), name, arguments), &mut env);
                    if outcome.ok { Ok(outcome) } else { Err(outcome.result) }
                }
            };
            let mut outcome = match outcome {
                Ok(outcome) => outcome,
                Err(result) => {
                    if let Some(before) = env.before.take() {
                        self.doc.model = before;
                    }
                    agent_restore_scope(&mut self.doc.model, scope);
                    return batch::report(calls, failure(result, results), batch);
                }
            };
            created.extend(outcome.created);
            modified.extend(outcome.modified);
            if name == "get_scene" {
                outcome.result["project"] = json!({"name": self.doc.name});
            }
            results.push(outcome.result);
        }
        let selection = calls.iter().any(|(name, _)| name == "select").then(|| env.selection.clone());
        if dry_run {
            if let Some(before) = env.before.take() {
                self.doc.model = before;
            }
            agent_restore_scope(&mut self.doc.model, scope);
            return batch::report(calls, batch::Applied::Ok(results), batch);
        }
        let Some(mut before) = env.before.take() else {
            // Nothing was edited (reads, a selection, or a no-op).
            agent_restore_scope(&mut self.doc.model, scope);
            if let Some(selection) = selection {
                self.apply_agent_selection(selection);
            }
            ctx.request_repaint();
            return batch::report(calls, batch::Applied::Ok(results), batch);
        };
        // The snapshot belongs to the scope the designer was in, so undo
        // leaves them there whatever enter_object did meanwhile.
        before.edit_context = scope;
        agent_restore_scope(&mut self.doc.model, scope);
        // One full comparison: the model is compared once here, not again
        // while recording.
        if before != self.doc.model && !self.commit_changed_edit(label, before, ctx) {
            // Shared edit access lapsed: rolled back whole.
            return (json!({"ok": false, "error": self.shared_edit_block_reason(ctx)}), false);
        }
        self.focus_touched(created.into_iter().chain(modified));
        if let Some(selection) = selection {
            self.apply_agent_selection(selection);
        }
        ctx.request_repaint();
        batch::report(calls, batch::Applied::Ok(results), batch)
    }

    /// Selects what an agent just made or changed (top-level, visible entities).
    fn focus_touched(&mut self, touched: impl IntoIterator<Item = u64>) {
        let model = &self.doc.model;
        let focus: BTreeSet<u64> = touched
            .into_iter()
            .filter(|id| model.entity_in_context(*id) == Some(*id) && !model.is_hidden(*id))
            .collect();
        if !focus.is_empty() && !(self.account.sharing_available() && self.doc.shared_project.is_some()) {
            self.selected = focus;
        }
    }

    /// Follows the selection an agent asked for with the `select` tool.
    fn apply_agent_selection(&mut self, selection: BTreeSet<u64>) {
        self.selected = selection
            .into_iter()
            .filter(|id| self.doc.model.contains_entity(id) && !self.doc.model.is_hidden(*id))
            .collect();
    }

    fn agent_undo(&mut self, redo: bool, ctx: &egui::Context) -> (bool, Value) {
        let label = if redo { self.doc.history.redo_label() } else { self.doc.history.undo_label() }.map(str::to_owned);
        let Some(label) = label else {
            let error = if redo { tr!("agent_link.error.nothing_to_redo") } else { tr!("agent_link.error.nothing_to_undo") };
            return (false, json!({"ok": false, "error": error}));
        };
        if !self.shared_edit_allowed(ctx) {
            return (false, json!({"ok": false, "error": self.shared_edit_block_reason(ctx)}));
        }
        let position = self.doc.history.position();
        self.undo(ctx, redo);
        if self.doc.history.position() == position {
            return (false, json!({"ok": false, "error": self.shared_edit_block_reason(ctx)}));
        }
        let key = if redo { "redone" } else { "undone" };
        let mut result = json!({"ok": true});
        result[key] = json!(label);
        (true, result)
    }

}

/// The answer to `render_view` for `model`.
fn render_answer(model: &Model, arguments: &Value) -> (bool, Value) {
    let view = arguments.get("view").and_then(Value::as_str).unwrap_or("iso");
    let size =
        |key: &str, default: u32| arguments.get(key).and_then(Value::as_u64).map_or(default, |v| v.min(u64::from(u32::MAX)) as u32);
    match benchup_tools::render::render(model, view, size("width", 1000), size("height", 750)) {
        Ok(rendered) => {
            (true, json!({"ok": true, "png_base64": benchup_tools::render::base64(&rendered.png), "info": rendered.info}))
        }
        Err(error) => (false, json!({"ok": false, "error": error})),
    }
}

/// Puts the designer back in the scope they had before an agent acted, or at
/// the top level if that object is gone.
fn agent_restore_scope(model: &mut Model, scope: Option<u64>) {
    model.edit_context = scope.filter(|id| model.objects.contains_key(id));
}
