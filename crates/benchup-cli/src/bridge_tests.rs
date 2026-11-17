use super::*;
use std::net::TcpStream;
use tungstenite::client::IntoClientRequest;

const TOKEN: &str = "0123456789abcdef-test-token";

fn config() -> Config {
    let mut config = Config::new(TOKEN);
    config.port = 0;
    config.app_wait = Duration::from_millis(300);
    config.call_timeout = Duration::from_secs(5);
    config
}

/// Opens a WebSocket to the bridge, optionally with a browser `Origin`.
fn socket(port: u16, origin: Option<&str>) -> Result<WebSocket<TcpStream>, String> {
    let stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut request = format!("ws://127.0.0.1:{port}/").into_client_request().unwrap();
    if let Some(origin) = origin {
        request.headers_mut().insert("origin", origin.parse().unwrap());
    }
    tungstenite::client::client(request, stream).map(|(ws, _)| ws).map_err(|e| e.to_string())
}

fn read(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        match ws.read().expect("a frame") {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            _ => continue,
        }
    }
}

fn send(ws: &mut WebSocket<TcpStream>, frame: Value) {
    ws.send(Message::text(frame.to_string())).unwrap();
}

/// Pairs as the app (as BenchUp does) and returns the socket once `ready`
/// has been verified, or the bridge's error frame.
fn pair_app(port: u16, token: &str, origin: Option<&str>) -> Result<WebSocket<TcpStream>, Value> {
    let mut ws = socket(port, origin).map_err(|e| json!({"error": e}))?;
    send(&mut ws, json!({"type": "authenticate", "ticket": "", "connection": "benchup-app-test"}));
    let challenge = read(&mut ws);
    assert_eq!(challenge["type"], "challenge");
    assert_eq!(challenge["protocol"], pairing::PROTOCOL);
    let bridge_nonce = challenge["nonce"].as_str().unwrap().to_owned();
    let nonce = "ab".repeat(32);
    send(&mut ws, json!({"type": "prove", "nonce": nonce, "proof": pairing::app_proof(token, &bridge_nonce, &nonce)}));
    let ready = read(&mut ws);
    if ready["type"] != "ready" {
        return Err(ready);
    }
    assert!(pairing::proofs_match(ready["proof"].as_str().unwrap(), &pairing::bridge_proof(token, &bridge_nonce, &nonce)));
    Ok(ws)
}

/// A fake BenchUp that answers every call with what it received.
fn echo_app(hub: &Hub) -> std::thread::JoinHandle<()> {
    let mut ws = pair_app(hub.port(), TOKEN, Some(DEFAULT_ORIGIN)).unwrap();
    let started = Instant::now();
    while hub.app().is_none() {
        assert!(started.elapsed() < Duration::from_secs(5), "the app never registered");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::spawn(move || {
        loop {
            let Ok(Message::Text(text)) = ws.read() else { return };
            let frame: Value = serde_json::from_str(&text).unwrap();
            if frame["type"] != "call" {
                continue;
            }
            assert!(frame["deadline_ms"].as_u64().unwrap() > now_ms());
            let result = json!({"ok": true, "name": frame["name"], "arguments": frame["arguments"], "client": frame["client"]});
            send(&mut ws, json!({"type": "result", "id": frame["id"], "ok": true, "result": result}));
        }
    })
}

#[test]
fn origins_are_limited_to_the_web_app_and_local_servers() {
    assert!(origin_allowed(None, &[]));
    assert!(origin_allowed(Some(DEFAULT_ORIGIN), &[]));
    // The GitHub Pages address redirects there, so the app loads from it.
    assert!(origin_allowed(Some("https://benchup.io"), &[]));
    assert!(!origin_allowed(Some("https://benchup.io.evil.example"), &[]));
    assert!(!origin_allowed(Some("http://benchup.io"), &[]));
    assert!(origin_allowed(Some("http://localhost:8080"), &[]));
    assert!(origin_allowed(Some("http://127.0.0.1"), &[]));
    assert!(!origin_allowed(Some("https://evil.example"), &[]));
    assert!(!origin_allowed(Some("http://localhost.evil.example"), &[]));
    assert!(!origin_allowed(Some("http://localhost:"), &[]));
    assert!(!origin_allowed(Some("https://benchup3d.github.io.evil.example"), &[]));
    assert!(origin_allowed(Some("https://mine.example"), &["https://mine.example".into()]));
}

#[test]
fn base64_round_trips() {
    for bytes in [&b""[..], b"a", b"ab", b"abc", b"\x89PNG\r\n\x1a\n\x00\xff"] {
        assert_eq!(decode_base64(&benchup_tools::render::base64(bytes)).unwrap(), bytes);
    }
    assert!(decode_base64("abc").is_err());
}

#[test]
fn clients_reach_the_app_through_the_bridge() {
    let hub = Hub::start(config()).unwrap();
    let _app = echo_app(&hub);
    let client = Client::new(hub.port(), TOKEN, "benchup-cli");
    let (result, ok) = client.request("get_scene", json!({"depth": 0})).unwrap();
    assert!(ok, "{result}");
    assert_eq!(result["name"], "get_scene");
    assert_eq!(result["arguments"], json!({"depth": 0}));
    assert_eq!(result["client"], "benchup-cli");

    // MCP session tools become calls the app knows.
    let mut live = Live::new(Target::Client(client));
    live.set_client("claude-code");
    assert!(live.supports_activity());
    assert!(live.instructions().contains("agent_activity"));
    for phase in ["analyzing", "preparing", "finished"] {
        let (result, ok) = live.call("claude-code", "agent_activity", json!({"phase": phase}));
        assert!(ok);
        assert_eq!(result["name"], "agent_activity");
        assert_eq!(result["arguments"], json!({"phase": phase}));
        assert_eq!(result["client"], "claude-code");
    }
    let (result, _) = live.batch(vec![("create_board".into(), json!({"name": "A"}))], true);
    assert_eq!(result["name"], "run_batch");
    assert_eq!(result["arguments"], json!({"calls": [{"tool": "create_board", "arguments": {"name": "A"}}], "dry_run": true}));
    assert_eq!(result["client"], "claude-code");
    let (result, _) = live.history(true);
    assert_eq!((result["name"].clone(), result["arguments"].clone()), (json!("undo"), json!({"redo": true})));
}

#[test]
fn a_wrong_token_or_origin_is_refused() {
    let hub = Hub::start(config()).unwrap();
    let error = pair_app(hub.port(), "a-different-token-entirely", None).err().unwrap();
    assert_eq!(error["message"], "pairing token does not match");
    assert!(socket(hub.port(), Some("https://evil.example")).is_err(), "other web pages get HTTP 403");

    let client = Client::new(hub.port(), "a-different-token-entirely", "benchup-cli");
    let ClientError::Failed(message) = client.request("get_scene", json!({})).unwrap_err() else { panic!() };
    assert!(message.contains("pairing token does not match"), "{message}");
    assert!(hub.app().is_none());
}

#[test]
fn calls_without_benchup_fail_with_a_hint() {
    let hub = Hub::start(config()).unwrap();
    let (result, ok) = hub.forward("agent", "get_scene", json!({}));
    assert!(!ok);
    assert_eq!(result["error"], "BenchUp is not connected to the bridge");
    assert!(result["hint"].as_str().unwrap().contains("Agent connection"));
}

#[test]
fn a_call_waits_for_benchup_to_connect() {
    let mut config = config();
    config.app_wait = Duration::from_secs(5);
    let hub = Hub::start(config).unwrap();
    let late = hub.clone();
    let app = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        echo_app(&late)
    });
    let (result, ok) = hub.forward("agent", "get_scene", json!({}));
    assert!(ok, "{result}");
    drop(app);
}

#[test]
fn an_unanswered_call_times_out_and_is_cancelled() {
    let mut config = config();
    config.call_timeout = Duration::from_millis(300);
    let hub = Hub::start(config).unwrap();
    let mut ws = pair_app(hub.port(), TOKEN, None).unwrap();
    while hub.app().is_none() {
        std::thread::sleep(Duration::from_millis(10));
    }
    let (result, ok) = hub.forward("agent", "move", json!({}));
    assert!(!ok);
    assert_eq!(result["error"], "BenchUp did not answer in time");
    let call = read(&mut ws);
    assert_eq!(call["type"], "call");
    let cancel = read(&mut ws);
    assert_eq!((cancel["type"].clone(), cancel["id"].clone()), (json!("cancel"), call["id"].clone()));
}

#[test]
fn a_second_bridge_on_the_same_port_reports_it_in_use() {
    let hub = Hub::start(config()).unwrap();
    let mut second = config();
    second.port = hub.port();
    assert!(matches!(Hub::start(second), Err(StartError::InUse(_))));
}

#[test]
fn nothing_listening_is_reported_as_no_bridge() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let client = Client::new(port, TOKEN, "benchup-cli");
    assert_eq!(client.request("get_scene", json!({})).unwrap_err(), ClientError::NoBridge);
}

#[test]
fn a_second_app_is_refused_while_the_first_stays_connected() {
    let hub = Hub::start(config()).unwrap();
    let _app = echo_app(&hub);
    let error = pair_app(hub.port(), TOKEN, None).err().unwrap();
    assert_eq!(error["type"], "error");
    assert_eq!((error["code"].clone(), error["message"].clone()), (json!(APP_BUSY.code), json!(APP_BUSY.message)));
    // The first app still answers.
    let (result, ok) = hub.forward("agent", "get_scene", json!({}));
    assert!(ok, "{result}");
}

#[test]
fn the_next_app_connects_once_the_first_leaves() {
    let hub = Hub::start(config()).unwrap();
    let mut first = pair_app(hub.port(), TOKEN, None).unwrap();
    while hub.app().is_none() {
        std::thread::sleep(Duration::from_millis(10));
    }
    first.close(None).unwrap();
    let _ = first.flush();
    drop(first);
    let started = Instant::now();
    loop {
        match pair_app(hub.port(), TOKEN, None) {
            Ok(_) => break,
            Err(error) => {
                assert_eq!((error["code"].clone(), error["message"].clone()), (json!(APP_BUSY.code), json!(APP_BUSY.message)));
                assert!(started.elapsed() < Duration::from_secs(5), "the first app was never forgotten");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[test]
fn a_call_too_large_for_the_app_is_refused_without_dropping_it() {
    let hub = Hub::start(config()).unwrap();
    let _app = echo_app(&hub);
    let (result, ok) = hub.forward("agent", "get_scene", json!({"padding": "x".repeat(MAX_CALL_BYTES)}));
    assert!(!ok);
    assert!(result["error"].as_str().unwrap().contains("larger than BenchUp accepts"), "{result}");
    let (result, ok) = hub.forward("agent", "get_scene", json!({}));
    assert!(ok, "{result}");
}

#[test]
fn an_mcp_bridge_forwarding_through_another_takes_the_port_over_when_it_exits() {
    // The other bridge (say, the background one `--live` started) has exited.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut takeover = config();
    takeover.port = port;
    let mut live = Live::forwarding(Client::new(port, TOKEN, "agent"), takeover);
    let (result, ok) = live.forward("get_scene", json!({}));
    assert!(!ok);
    assert_eq!(result["error"], "BenchUp is not connected to the bridge", "it listens itself and waits for BenchUp");
    let Target::Hub(hub) = &live.target else { panic!("still forwarding to a bridge that is gone") };
    assert_eq!(hub.port(), port);

    // BenchUp reconnects to the same port and calls go through again.
    let _app = echo_app(hub);
    let (result, ok) = live.forward("get_scene", json!({}));
    assert!(ok, "{result}");
}
