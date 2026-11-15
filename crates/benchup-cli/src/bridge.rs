//! `benchup bridge`: lets agents edit the project open in BenchUp.
//!
//! ```text
//! agent ──MCP (stdio)──▶ benchup bridge (127.0.0.1) ◀──WebSocket── BenchUp (web or desktop)
//! benchup call --live ──WebSocket──┘
//! ```
//!
//! A browser tab cannot listen on a port, so BenchUp connects out to the
//! bridge, which listens on 127.0.0.1 only. Whoever connects proves it knows
//! the pairing token shown in BenchUp with a challenge-response
//! ([`benchup_tools::pairing`]); the token itself is never sent. Browsers must
//! also come from an allowed `Origin`, so other web pages cannot reach it.
//!
//! Two kinds of peers connect:
//! * the app (`role` absent or `"app"`): one at a time; it runs the calls it
//!   is sent and answers each with a result. While one is connected, another
//!   app is refused (before `ready`) rather than replacing it, so two BenchUp
//!   windows sharing a token do not keep pushing each other off;
//! * clients (`role: "client"`): `benchup call --live` and friends, or a
//!   second `benchup bridge` serving MCP while this one holds the port. Each
//!   sends `request` frames, which are forwarded to the app.
//!
//! Frames are JSON text. Handshake, for both kinds:
//! * peer: `{"type":"authenticate","connection":NAME,"role":"app"|"client"}`
//! * bridge: `{"type":"challenge","nonce":HEX64,"protocol":2,"server":"benchup-bridge","version":V}`
//! * peer: `{"type":"prove","nonce":HEX64,"proof":HMAC(token,"benchup-agent-link/<role>\n<bridge nonce>\n<peer nonce>")}`
//! * bridge: `{"type":"ready","proof":HMAC(token,"benchup-agent-link/bridge\n…")}`
//!
//! Then, bridge to app: `call {id, name, arguments, client, deadline_ms}` and
//! `cancel {id}`; app to bridge: `result {id, ok, result}` and `status
//! {message}`. Client to bridge: `request {id, name, arguments, client}`;
//! bridge to client: `response {id, ok, result}`. Besides the shared tools,
//! `name` may be `run_batch` (`{calls, dry_run, single}`, one undo step;
//! `single: true` with one call runs it as written, without `$N`
//! references, and answers with that call's own result), `undo`
//! (`{redo}`) or `render_view` (`{view, width, height}`, answered with
//! `png_base64` and `info`).
use crate::mcp::Backend;
use benchup_tools::pairing;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{
    Message, WebSocket,
    handshake::server::{ErrorResponse, Request, Response},
};

pub const DEFAULT_PORT: u16 = pairing::DEFAULT_PORT;
/// The published web app. Local development servers are always allowed too.
pub const DEFAULT_ORIGIN: &str = "https://benchup3d.github.io";
pub const TOKEN_ENV: &str = "BENCHUP_AGENT_TOKEN";
pub const PORT_ENV: &str = "BENCHUP_BRIDGE_PORT";
pub const MIN_TOKEN_LEN: usize = 16;
/// The handshake, WebSocket upgrade included, must finish within this long.
const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(25);
/// Largest message accepted before the peer has proved the pairing token.
const PRE_AUTH_MAX_BYTES: usize = 1024 * 1024;
/// Text frames before authentication are tiny; anything bigger is hostile.
const PRE_AUTH_TEXT_MAX: usize = 64 * 1024;
/// Largest message from an authenticated peer (scene dumps, rendered views).
const MAX_MESSAGE_BYTES: usize = pairing::MAX_BRIDGE_MESSAGE_BYTES;
/// Largest call forwarded to the app: it drops the whole connection on
/// anything bigger, so larger calls are refused here, one at a time.
const MAX_CALL_BYTES: usize = pairing::MAX_APP_MESSAGE_BYTES;
/// A refusal: a stable `code` the app translates, and English text for logs
/// and command-line clients.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Refusal {
    code: &'static str,
    message: &'static str,
}
/// Refusal sent to an app while another one is connected.
const APP_BUSY: Refusal =
    Refusal { code: pairing::refusal::APP_BUSY, message: "another BenchUp window is already connected to this bridge" };
const UNKNOWN_ROLE: Refusal = Refusal { code: pairing::refusal::UNKNOWN_ROLE, message: "unknown role" };
const TOKEN_MISMATCH: Refusal = Refusal { code: pairing::refusal::TOKEN_MISMATCH, message: "pairing token does not match" };
const UNEXPECTED: Refusal = Refusal { code: pairing::refusal::UNEXPECTED, message: "unexpected message during pairing" };
/// Connections that have not finished authenticating; extras are closed at once.
const MAX_UNAUTHENTICATED: usize = 8;
const SERVER: &str = "benchup-bridge";

#[derive(Clone, Debug)]
pub struct Config {
    /// 0 picks a free port (tests).
    pub port: u16,
    pub token: String,
    pub extra_origins: Vec<String>,
    /// How long the app may take to answer one call.
    pub call_timeout: Duration,
    /// How long a call waits for BenchUp to connect when it is not connected
    /// yet (the app retries every few seconds).
    pub app_wait: Duration,
}

impl Config {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            port: DEFAULT_PORT,
            token: token.into(),
            extra_origins: Vec::new(),
            call_timeout: Duration::from_secs(120),
            app_wait: Duration::from_secs(15),
        }
    }
}

/// The pairing token from the environment (never from the command line,
/// which other local users can read).
pub fn token_from_env() -> Result<String, String> {
    let token = std::env::var(TOKEN_ENV).unwrap_or_default();
    let token = token.trim();
    if token.is_empty() {
        return Err(format!(
            "set {TOKEN_ENV} to the pairing token shown in BenchUp (Settings → Agent connection)"
        ));
    }
    if token.len() < MIN_TOKEN_LEN {
        return Err(format!("{TOKEN_ENV} is too short to be a BenchUp pairing token"));
    }
    Ok(token.to_owned())
}

/// The bridge port: `BENCHUP_BRIDGE_PORT`, or the default.
pub fn port_from_env() -> Result<u16, String> {
    match std::env::var(PORT_ENV) {
        Ok(port) if !port.trim().is_empty() => {
            port.trim().parse().map_err(|_| format!("{PORT_ENV} must be a port number"))
        }
        _ => Ok(DEFAULT_PORT),
    }
}

fn random_nonce() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("the operating system has no secure randomness: {e}"))?;
    Ok(pairing::to_hex(&bytes))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Whether a WebSocket `Origin` may connect: none (a native program), the
/// published web app, a local development server, or one passed with
/// `--allow-origin`.
pub fn origin_allowed(origin: Option<&str>, extra: &[String]) -> bool {
    let Some(origin) = origin else { return true };
    if origin == DEFAULT_ORIGIN || extra.iter().any(|o| o == origin) {
        return true;
    }
    ["http://localhost", "http://127.0.0.1"].iter().any(|base| {
        origin.strip_prefix(base).is_some_and(|rest| {
            rest.is_empty()
                || rest.strip_prefix(':').is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        })
    })
}

/// Why a forwarded call has no answer.
#[derive(Debug, PartialEq)]
enum Failure {
    NotConnected,
    Timeout,
    Disconnected,
    TooLarge,
}

fn failure_answer(failure: Failure) -> (Value, bool) {
    let value = match failure {
        Failure::NotConnected => json!({"ok": false, "error": "BenchUp is not connected to the bridge",
            "hint": "Open BenchUp, go to Settings → Agent connection, turn on Allow agent connection and check that the pairing token matches BENCHUP_AGENT_TOKEN."}),
        Failure::Timeout => json!({"ok": false, "error": "BenchUp did not answer in time",
            "hint": "The call was cancelled. The designer may be asked to grant access; call get_scene to check the current state before retrying."}),
        Failure::Disconnected => json!({"ok": false, "error": "BenchUp disconnected before answering",
            "hint": "Check with get_scene whether the change was applied once BenchUp reconnects."}),
        Failure::TooLarge => json!({"ok": false, "error": format!("the call is larger than BenchUp accepts ({} MB)", MAX_CALL_BYTES / (1024 * 1024)),
            "hint": "Split the work into several smaller calls or batches."}),
    };
    (value, false)
}

struct App {
    id: u64,
    tx: Sender<String>,
    #[cfg_attr(not(test), allow(dead_code))]
    name: String,
}

#[derive(Default)]
struct Shared {
    app: Option<App>,
    next_conn: u64,
    next_call: u64,
    /// call id -> (app connection id, where to deliver the answer)
    pending: HashMap<String, (u64, Sender<Result<(Value, bool), Failure>>)>,
    last_activity: Option<Instant>,
}

/// Counts a connection against the unauthenticated-connection cap until released.
struct Slot(Option<Arc<AtomicUsize>>);
impl Slot {
    fn release(&mut self) {
        if let Some(counter) = self.0.take() {
            counter.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        self.release();
    }
}

/// A stream whose reads stop at a deadline, so slow or silent peers cannot
/// hold a connection slot, and whose poll interval is adjustable.
struct Guarded {
    stream: TcpStream,
    deadline: Option<Instant>,
    poll: Duration,
}
impl Read for Guarded {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut wait = self.poll;
        if let Some(deadline) = self.deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            wait = wait.min(remaining);
        }
        self.stream.set_read_timeout(Some(wait))?;
        self.stream.read(buf)
    }
}
impl Write for Guarded {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// Why the bridge could not start.
#[derive(Debug)]
pub enum StartError {
    /// Another program, usually another bridge, holds the port.
    InUse(u16),
    Other(String),
}

impl std::fmt::Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InUse(port) => write!(f, "port {port} on 127.0.0.1 is already in use (is another bridge running?)"),
            Self::Other(message) => f.write_str(message),
        }
    }
}

struct Inner {
    shared: Mutex<Shared>,
    config: Config,
}

/// The listening bridge. Cloning shares it.
#[derive(Clone)]
pub struct Hub {
    inner: Arc<Inner>,
    port: u16,
}

impl Hub {
    /// Binds 127.0.0.1 only and accepts connections in the background.
    pub fn start(config: Config) -> Result<Self, StartError> {
        if config.token.len() < MIN_TOKEN_LEN {
            return Err(StartError::Other(format!("the pairing token must be at least {MIN_TOKEN_LEN} characters")));
        }
        random_nonce().map_err(StartError::Other)?; // refuse to start without OS randomness
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, config.port))).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                StartError::InUse(config.port)
            } else {
                StartError::Other(format!("cannot listen on 127.0.0.1:{}: {e}", config.port))
            }
        })?;
        let port = listener.local_addr().map_err(|e| StartError::Other(e.to_string()))?.port();
        let inner = Arc::new(Inner { shared: Mutex::new(Shared::default()), config });
        let accept = inner.clone();
        let unauthenticated = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if unauthenticated.fetch_add(1, Ordering::SeqCst) >= MAX_UNAUTHENTICATED {
                    unauthenticated.fetch_sub(1, Ordering::SeqCst);
                    continue; // dropping the stream closes it
                }
                let slot = Slot(Some(unauthenticated.clone()));
                let inner = accept.clone();
                std::thread::spawn(move || handle_connection(stream, inner, slot));
            }
        });
        Ok(Self { inner, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The connected app's name, if one is connected.
    #[cfg(test)]
    pub fn app(&self) -> Option<String> {
        self.inner.shared.lock().unwrap().app.as_ref().map(|app| app.name.clone())
    }

    /// When a call last went through, for `--idle-exit`.
    fn idle_for(&self) -> Option<Duration> {
        self.inner.shared.lock().unwrap().last_activity.map(|at| at.elapsed())
    }

    /// Sends one call to the app and waits for its answer. When no app is
    /// connected, waits up to `app_wait` for one.
    pub fn forward(&self, client: &str, name: &str, arguments: Value) -> (Value, bool) {
        match self.try_forward(client, name, arguments) {
            Ok(answer) => answer,
            Err(failure) => failure_answer(failure),
        }
    }

    fn try_forward(&self, client: &str, name: &str, arguments: Value) -> Result<(Value, bool), Failure> {
        let config = &self.inner.config;
        let waited = Instant::now();
        let (answer_tx, answer_rx) = mpsc::channel();
        let (call_id, tx) = loop {
            {
                let mut shared = self.inner.shared.lock().unwrap();
                shared.last_activity = Some(Instant::now());
                if let Some(app) = &shared.app {
                    let (conn, tx) = (app.id, app.tx.clone());
                    shared.next_call += 1;
                    let call_id = format!("c{}", shared.next_call);
                    let deadline_ms = now_ms() + config.call_timeout.as_millis() as u64;
                    let frame = json!({"type": "call", "id": call_id, "name": name, "arguments": arguments,
                        "client": client, "deadline_ms": deadline_ms})
                    .to_string();
                    if frame.len() > MAX_CALL_BYTES {
                        return Err(Failure::TooLarge);
                    }
                    if tx.send(frame).is_ok() {
                        shared.pending.insert(call_id.clone(), (conn, answer_tx));
                        break (call_id, tx);
                    }
                }
            }
            if waited.elapsed() >= config.app_wait {
                return Err(Failure::NotConnected);
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let answer = match answer_rx.recv_timeout(config.call_timeout) {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => {
                // A result that arrives later finds no pending call and is ignored.
                self.inner.shared.lock().unwrap().pending.remove(&call_id);
                let _ = tx.send(json!({"type": "cancel", "id": call_id}).to_string());
                Err(Failure::Timeout)
            }
            Err(RecvTimeoutError::Disconnected) => Err(Failure::Disconnected),
        };
        self.inner.shared.lock().unwrap().last_activity = Some(Instant::now());
        answer
    }

    /// Exits the process once no call has gone through for `idle` (the
    /// bridge `benchup call --live` starts on its own does not linger).
    pub fn exit_when_idle(&self, idle: Duration) {
        self.inner.shared.lock().unwrap().last_activity.get_or_insert_with(Instant::now);
        let hub = self.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1).min(idle));
                if hub.idle_for().is_some_and(|quiet| quiet >= idle) {
                    eprintln!("benchup bridge: no calls for {} s, exiting", idle.as_secs());
                    std::process::exit(0);
                }
            }
        });
    }
}

fn is_timeout(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(e)
        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    App,
    Client,
}

fn reject(ws: &mut WebSocket<Guarded>, refusal: Refusal) {
    let frame = json!({"type": "error", "code": refusal.code, "message": refusal.message});
    let _ = ws.send(Message::text(frame.to_string()));
    let _ = ws.close(None);
    let _ = ws.flush();
}

/// The pairing handshake: who connected, or `None` (the socket is closed).
/// Once the peer has proved the token, `admit` decides whether it may stay;
/// a refusal is sent instead of `ready`.
fn authenticate(
    ws: &mut WebSocket<Guarded>,
    token: &str,
    deadline: Instant,
    admit: &mut dyn FnMut(Role, &str) -> Result<(), Refusal>,
) -> Option<(Role, String)> {
    let bridge_nonce = random_nonce().ok()?;
    let mut hello: Option<(Role, String)> = None;
    loop {
        // Checked on every iteration so traffic cannot stretch the window.
        if Instant::now() >= deadline {
            return None;
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                if text.len() > PRE_AUTH_TEXT_MAX {
                    return None;
                }
                let frame: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match (&hello, frame.get("type").and_then(Value::as_str)) {
                    (None, Some("authenticate")) => {
                        let role = match frame.get("role").and_then(Value::as_str) {
                            None | Some("app") => Role::App,
                            Some("client") => Role::Client,
                            Some(_) => {
                                reject(ws, UNKNOWN_ROLE);
                                return None;
                            }
                        };
                        let name: String = frame
                            .get("connection")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(pairing::MAX_CLIENT_CHARS)
                            .collect();
                        let challenge = json!({"type": "challenge", "nonce": bridge_nonce, "server": SERVER,
                            "version": env!("CARGO_PKG_VERSION"), "protocol": pairing::PROTOCOL});
                        ws.send(Message::text(challenge.to_string())).ok()?;
                        hello = Some((role, name));
                    }
                    (Some((role, name)), Some("prove")) => {
                        let peer_nonce = frame.get("nonce").and_then(Value::as_str).unwrap_or("");
                        let proof = frame.get("proof").and_then(Value::as_str).unwrap_or("");
                        let expected = match role {
                            Role::App => pairing::app_proof(token, &bridge_nonce, peer_nonce),
                            Role::Client => pairing::client_proof(token, &bridge_nonce, peer_nonce),
                        };
                        if !pairing::valid_nonce(peer_nonce) || !pairing::proofs_match(proof, &expected) {
                            reject(ws, TOKEN_MISMATCH);
                            return None;
                        }
                        if let Err(refusal) = admit(*role, name) {
                            reject(ws, refusal);
                            return None;
                        }
                        let ready = json!({"type": "ready", "server": SERVER, "version": env!("CARGO_PKG_VERSION"),
                            "protocol": pairing::PROTOCOL,
                            "proof": pairing::bridge_proof(token, &bridge_nonce, peer_nonce)});
                        ws.send(Message::text(ready.to_string())).ok()?;
                        return Some((*role, name.clone()));
                    }
                    _ => {
                        reject(ws, UNEXPECTED);
                        return None;
                    }
                }
            }
            Ok(Message::Close(_)) => return None,
            // Before the hello these are ignored (and never extend the deadline);
            // afterwards anything but the proof is a protocol error.
            Ok(_) if hello.is_some() => {
                reject(ws, UNEXPECTED);
                return None;
            }
            Ok(_) => {}
            Err(e) if is_timeout(&e) => {}
            Err(_) => return None,
        }
    }
}

fn handle_connection(stream: TcpStream, inner: Arc<Inner>, mut slot: Slot) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_write_timeout(Some(AUTH_TIMEOUT));
    let deadline = Instant::now() + AUTH_TIMEOUT;
    let guarded = Guarded { stream, deadline: Some(deadline), poll: AUTH_TIMEOUT };
    let extra_origins = inner.config.extra_origins.clone();
    let check_origin = |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        let origin = request.headers().get("origin").map(|v| v.to_str().unwrap_or("\u{0}invalid"));
        if origin_allowed(origin, &extra_origins) {
            Ok(response)
        } else {
            eprintln!("benchup bridge: refused a connection from origin {origin:?}");
            let mut rejection = ErrorResponse::new(Some("origin not allowed".to_owned()));
            *rejection.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
            Err(rejection)
        }
    };
    let limits =
        WebSocketConfig::default().max_message_size(Some(PRE_AUTH_MAX_BYTES)).max_frame_size(Some(PRE_AUTH_MAX_BYTES));
    let Ok(mut ws) = tungstenite::accept_hdr_with_config(guarded, check_origin, Some(limits)) else { return };
    ws.get_mut().poll = POLL;
    // The app is registered while pairing, atomically, so a second one is
    // refused instead of replacing it.
    let mut registered: Option<(u64, Receiver<String>)> = None;
    let mut admit = |role: Role, name: &str| -> Result<(), Refusal> {
        if role == Role::Client {
            return Ok(());
        }
        let mut guard = inner.shared.lock().unwrap();
        if guard.app.is_some() {
            return Err(APP_BUSY);
        }
        let (tx, rx) = mpsc::channel();
        guard.next_conn += 1;
        let id = guard.next_conn;
        guard.app = Some(App { id, tx, name: name.to_owned() });
        registered = Some((id, rx));
        Ok(())
    };
    let authenticated = authenticate(&mut ws, &inner.config.token, deadline, &mut admit);
    let Some((role, name)) = authenticated else {
        // Registered, but the `ready` could not be sent.
        if let Some((id, _)) = registered {
            unregister_app(&inner, id);
        }
        return;
    };
    // Authenticated: the unauthenticated slot and the small limits go away.
    slot.release();
    ws.get_mut().deadline = None;
    let _ = ws.get_mut().stream.set_write_timeout(Some(Duration::from_secs(30)));
    ws.set_config(|c| {
        c.max_message_size = Some(MAX_MESSAGE_BYTES);
        c.max_frame_size = Some(MAX_MESSAGE_BYTES);
    });
    match (role, registered) {
        (Role::App, Some((conn_id, rx))) => serve_app(ws, &inner, &name, conn_id, rx),
        (Role::App, None) => {}
        (Role::Client, _) => serve_client(ws, Hub { inner, port: 0 }, name),
    }
}

/// Forgets the app connection `conn_id`, if it is still the registered one,
/// and fails the calls it was asked.
fn unregister_app(inner: &Inner, conn_id: u64) {
    let mut guard = inner.shared.lock().unwrap();
    if guard.app.as_ref().is_some_and(|a| a.id == conn_id) {
        guard.app = None;
        eprintln!("benchup bridge: BenchUp disconnected");
    }
    let lost: Vec<String> = guard.pending.iter().filter(|(_, (c, _))| *c == conn_id).map(|(k, _)| k.clone()).collect();
    for key in lost {
        if let Some((_, answer)) = guard.pending.remove(&key) {
            let _ = answer.send(Err(Failure::Disconnected));
        }
    }
}

fn serve_app(mut ws: WebSocket<Guarded>, inner: &Inner, name: &str, conn_id: u64, rx: Receiver<String>) {
    eprintln!("benchup bridge: BenchUp connected ({name})");
    'connection: loop {
        loop {
            match rx.try_recv() {
                Ok(text) => {
                    if ws.send(Message::text(text)).is_err() {
                        break 'connection;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // No longer registered: close.
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    break 'connection;
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => handle_app_frame(&text, conn_id, &inner.shared),
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(e) if is_timeout(&e) => {}
            Err(_) => break,
        }
    }
    unregister_app(inner, conn_id);
}

fn handle_app_frame(text: &str, conn_id: u64, shared: &Mutex<Shared>) {
    let Ok(frame) = serde_json::from_str::<Value>(text) else { return };
    match frame.get("type").and_then(Value::as_str) {
        Some("result") => {
            let Some(id) = frame.get("id").and_then(Value::as_str) else { return };
            let mut guard = shared.lock().unwrap();
            // Only the connection that was asked may answer.
            if guard.pending.get(id).is_some_and(|(c, _)| *c == conn_id)
                && let Some((_, answer)) = guard.pending.remove(id)
            {
                let ok = frame.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let _ = answer.send(Ok((frame.get("result").cloned().unwrap_or(Value::Null), ok)));
            }
        }
        Some("status") => {
            let message: String =
                frame.get("message").and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control()).take(200).collect();
            eprintln!("benchup bridge: BenchUp: {message}");
        }
        _ => {}
    }
}

/// A command-line client: each `request` is forwarded to the app in turn.
fn serve_client(mut ws: WebSocket<Guarded>, hub: Hub, name: String) {
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let frame: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if frame.get("type").and_then(Value::as_str) != Some("request") {
                    continue;
                }
                let id = frame.get("id").cloned().unwrap_or(Value::Null);
                let (result, ok) = match frame.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) {
                    None => (json!({"ok": false, "error": "the request names no tool"}), false),
                    Some(tool) => {
                        let arguments = frame.get("arguments").cloned().unwrap_or_else(|| json!({}));
                        let client = frame.get("client").and_then(Value::as_str).unwrap_or(&name);
                        hub.forward(client, tool, arguments)
                    }
                };
                let response = json!({"type": "response", "id": id, "ok": ok, "result": result});
                if ws.send(Message::text(response.to_string())).is_err() {
                    return;
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(e) if is_timeout(&e) => {}
            Err(_) => return,
        }
    }
}

/// Talks to a running bridge as a client.
#[derive(Clone, Debug)]
pub struct Client {
    pub port: u16,
    pub token: String,
    /// Shown to the designer as the agent's name.
    pub name: String,
    /// How long to wait for one answer (the bridge's own limits plus a margin).
    pub timeout: Duration,
}

/// Why a client request failed before the bridge answered.
#[derive(Debug, PartialEq)]
pub enum ClientError {
    /// Nothing listens on the port.
    NoBridge,
    Failed(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBridge => f.write_str("no bridge is running"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl Client {
    pub fn new(port: u16, token: impl Into<String>, name: impl Into<String>) -> Self {
        Self { port, token: token.into(), name: name.into(), timeout: Duration::from_secs(150) }
    }

    fn read_frame(ws: &mut WebSocket<TcpStream>) -> Result<Value, ClientError> {
        loop {
            match ws.read() {
                Ok(Message::Text(text)) => {
                    return serde_json::from_str(&text).map_err(|e| ClientError::Failed(format!("bad frame from the bridge: {e}")));
                }
                Ok(Message::Close(_)) => return Err(ClientError::Failed("the bridge closed the connection".into())),
                Ok(_) => {}
                Err(e) if is_timeout(&e) => return Err(ClientError::Failed("the bridge did not answer in time".into())),
                Err(e) => return Err(ClientError::Failed(format!("lost the bridge: {e}"))),
            }
        }
    }

    /// Connects and pairs; the bridge must prove it knows the token too.
    fn connect(&self) -> Result<WebSocket<TcpStream>, ClientError> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.port));
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|e| match e.kind() {
            std::io::ErrorKind::ConnectionRefused => ClientError::NoBridge,
            _ => ClientError::Failed(format!("cannot reach the bridge on 127.0.0.1:{}: {e}", self.port)),
        })?;
        let _ = stream.set_nodelay(true);
        stream.set_read_timeout(Some(AUTH_TIMEOUT)).map_err(|e| ClientError::Failed(e.to_string()))?;
        let url = format!("ws://127.0.0.1:{}/", self.port);
        let limits = WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE_BYTES * 2)).max_frame_size(Some(MAX_MESSAGE_BYTES * 2));
        let (mut ws, _) = tungstenite::client::client_with_config(url.as_str(), stream, Some(limits))
            .map_err(|e| ClientError::Failed(format!("the bridge refused the connection: {e}")))?;
        let send = |ws: &mut WebSocket<TcpStream>, frame: Value| {
            ws.send(Message::text(frame.to_string())).map_err(|e| ClientError::Failed(format!("lost the bridge: {e}")))
        };
        send(&mut ws, json!({"type": "authenticate", "role": "client", "connection": self.name}))?;
        let challenge = Self::read_frame(&mut ws)?;
        let bridge_nonce = challenge.get("nonce").and_then(Value::as_str).unwrap_or_default().to_owned();
        if challenge.get("type").and_then(Value::as_str) != Some("challenge")
            || challenge.get("protocol").and_then(Value::as_u64) != Some(u64::from(pairing::PROTOCOL))
            || !pairing::valid_nonce(&bridge_nonce)
        {
            return Err(ClientError::Failed(format!(
                "something other than a compatible benchup bridge listens on 127.0.0.1:{}",
                self.port
            )));
        }
        let nonce = random_nonce().map_err(ClientError::Failed)?;
        let proof = pairing::client_proof(&self.token, &bridge_nonce, &nonce);
        send(&mut ws, json!({"type": "prove", "nonce": nonce, "proof": proof}))?;
        let ready = Self::read_frame(&mut ws)?;
        match ready.get("type").and_then(Value::as_str) {
            Some("ready") => {}
            Some("error") => {
                return Err(ClientError::Failed(format!(
                    "the bridge refused this client: {}. Is {TOKEN_ENV} the token shown in BenchUp?",
                    ready.get("message").and_then(Value::as_str).unwrap_or("pairing failed")
                )));
            }
            _ => return Err(ClientError::Failed("unexpected answer from the bridge while pairing".into())),
        }
        let expected = pairing::bridge_proof(&self.token, &bridge_nonce, &nonce);
        if !ready.get("proof").and_then(Value::as_str).is_some_and(|proof| pairing::proofs_match(proof, &expected)) {
            return Err(ClientError::Failed("the bridge could not prove it knows the pairing token".into()));
        }
        ws.get_mut().set_read_timeout(Some(self.timeout)).map_err(|e| ClientError::Failed(e.to_string()))?;
        Ok(ws)
    }

    /// Runs one call in the open app through the bridge.
    pub fn request(&self, name: &str, arguments: Value) -> Result<(Value, bool), ClientError> {
        let mut ws = self.connect()?;
        let request = json!({"type": "request", "id": 1, "name": name, "arguments": arguments, "client": self.name});
        ws.send(Message::text(request.to_string())).map_err(|e| ClientError::Failed(format!("lost the bridge: {e}")))?;
        let response = Self::read_frame(&mut ws)?;
        let _ = ws.close(None);
        let _ = ws.flush();
        if response.get("type").and_then(Value::as_str) != Some("response") {
            return Err(ClientError::Failed("unexpected answer from the bridge".into()));
        }
        let ok = response.get("ok").and_then(Value::as_bool).unwrap_or(false);
        Ok((response.get("result").cloned().unwrap_or(Value::Null), ok))
    }
}

/// How long a bridge started by `benchup call --live` waits for calls
/// before exiting.
pub const AUTO_IDLE: Duration = Duration::from_secs(30 * 60);

/// `--live` commands: runs `name` in the open app. Starts a bridge in the
/// background when none is running, so CLI-first agents need no setup
/// beyond `BENCHUP_AGENT_TOKEN`.
pub fn cli_request(name: &str, arguments: Value) -> Result<(Value, bool), String> {
    let client = Client::new(port_from_env()?, token_from_env()?, "benchup-cli");
    match client.request(name, arguments.clone()) {
        Err(ClientError::NoBridge) => {}
        other => return other.map_err(|e| e.to_string()),
    }
    start_background_bridge(client.port)?;
    let started = Instant::now();
    loop {
        match client.request(name, arguments.clone()) {
            Err(ClientError::NoBridge) if started.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(ClientError::NoBridge) => return Err("the background bridge did not start; run `benchup bridge --no-mcp` to see why".into()),
            other => return other.map_err(|e| e.to_string()),
        }
    }
}

fn start_background_bridge(port: u16) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find the benchup executable: {e}"))?;
    std::process::Command::new(exe)
        .args(["bridge", "--no-mcp", "--port", &port.to_string(), "--idle-exit", &AUTO_IDLE.as_secs().to_string()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start a bridge: {e}"))?;
    Ok(())
}

/// Decodes standard base64 (rendered views come back as `png_base64`).
pub fn decode_base64(text: &str) -> Result<Vec<u8>, String> {
    let value = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("invalid base64".into()),
        } as u32)
    };
    let bytes: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return Err("invalid base64 length".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 {
            return Err("invalid base64 padding".into());
        }
        let mut n = 0u32;
        for &c in &chunk[..4 - pad] {
            n = (n << 6) | value(c)?;
        }
        n <<= 6 * pad as u32;
        let decoded = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&decoded[..3 - pad]);
    }
    Ok(out)
}

/// Where MCP calls go in live mode: this process's own bridge, or the one
/// already holding the port.
pub enum Target {
    Hub(Hub),
    Client(Client),
}

/// The MCP backend of `benchup bridge`.
pub struct Live {
    pub target: Target,
    /// While forwarding through another bridge: how to start this process's
    /// own once that one is gone (the bridge `--live` starts in the
    /// background exits when idle).
    takeover: Option<Config>,
    /// The MCP client's name (from `initialize`), shown to the designer.
    client: String,
}

impl Live {
    pub fn new(target: Target) -> Self {
        Self { target, takeover: None, client: "agent".into() }
    }

    /// Forwards through the bridge that holds the port, and listens on the
    /// port itself with `config` if that bridge exits.
    pub fn forwarding(client: Client, config: Config) -> Self {
        Self { target: Target::Client(client), takeover: Some(config), client: "agent".into() }
    }

    /// The bridge calls were forwarded through is gone: listen on its port.
    fn take_over(&mut self) -> bool {
        let Some(config) = &self.takeover else { return false };
        match Hub::start(config.clone()) {
            Ok(hub) => {
                eprintln!("benchup bridge: the other bridge exited; waiting for BenchUp on ws://127.0.0.1:{}", hub.port());
                self.target = Target::Hub(hub);
                self.takeover = None;
                true
            }
            Err(_) => false,
        }
    }
}

const LIVE_INSTRUCTIONS: &str = "Edits the BenchUp 3D woodworking project open in the app, live, while the designer watches. \
Lengths are millimetres, Z is up. Call get_scene first to see the model. Each changing call (and each run_batch) is one undo \
step labelled \"Agent: …\" in BenchUp's history. Every call starts in the scope the designer is in, so put enter_object and \
the calls that need it in one run_batch. The current selection is what the designer has selected, so \"this\" usually means \
get_selection. When the designer is in the middle of a tool or has taken control, they are asked to grant access first; a \
refusal comes back as an error: wait and ask them before retrying. Shared projects keep their read-only state and edit leases. \
Call get_guide once for conventions (front is -y), joint roles and woodworking recipes.";

impl Live {
    fn forward(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let bridge = match &self.target {
            Target::Hub(hub) => return hub.forward(&self.client, name, arguments),
            Target::Client(bridge) => Client { name: self.client.clone(), ..bridge.clone() },
        };
        let mut answer = bridge.request(name, arguments.clone());
        if answer == Err(ClientError::NoBridge) {
            if self.take_over()
                && let Target::Hub(hub) = &self.target
            {
                return hub.forward(&self.client, name, arguments);
            }
            // Another process took the port first: go through it.
            answer = bridge.request(name, arguments);
        }
        answer.unwrap_or_else(|error| (json!({"ok": false, "error": format!("cannot reach the bridge: {error}")}), false))
    }
}

impl Backend for Live {
    fn call(&mut self, _client: &str, name: &str, arguments: Value) -> (Value, bool) {
        self.forward(name, arguments)
    }

    fn set_client(&mut self, client: &str) {
        self.client = client.chars().filter(|c| !c.is_control()).take(pairing::MAX_CLIENT_CHARS).collect();
    }

    fn batch(&mut self, calls: Vec<(String, Value)>, dry_run: bool) -> (Value, bool) {
        let calls: Vec<Value> = calls.into_iter().map(|(tool, arguments)| json!({"tool": tool, "arguments": arguments})).collect();
        self.forward("run_batch", json!({"calls": calls, "dry_run": dry_run}))
    }

    fn history(&mut self, redo: bool) -> (Value, bool) {
        self.forward("undo", json!({"redo": redo}))
    }

    fn render(&mut self, view: &str, width: u32, height: u32) -> Result<(String, Value), String> {
        let (mut result, ok) = self.forward("render_view", json!({"view": view, "width": width, "height": height}));
        if !ok {
            return Err(result.get("error").and_then(Value::as_str).unwrap_or("BenchUp could not render the view").to_owned());
        }
        // Passed on as it came: MCP wants base64 too.
        match result.get_mut("png_base64").map(Value::take) {
            Some(Value::String(png)) if !png.is_empty() => Ok((png, result.get("info").cloned().unwrap_or(Value::Null))),
            _ => Err("BenchUp sent no image".into()),
        }
    }

    fn instructions(&self) -> &'static str {
        LIVE_INSTRUCTIONS
    }
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
