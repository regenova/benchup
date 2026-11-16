//! `benchup bridge` and `--live` commands against a stand-in for BenchUp.
use benchup_tools::pairing;
use serde_json::{Value, json};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

const TOKEN: &str = "0123456789abcdef-live-test";

struct Bridge(Child);
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn benchup(port: u16, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_benchup"));
    command.args(args).env("BENCHUP_AGENT_TOKEN", TOKEN).env("BENCHUP_BRIDGE_PORT", port.to_string());
    command
}

fn run(port: u16, args: &[&str]) -> (Value, Output) {
    let output = benchup(port, args).stdin(Stdio::null()).output().unwrap();
    let value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&output.stderr)));
    (value, output)
}

fn read(ws: &mut WebSocket<TcpStream>) -> Value {
    loop {
        if let Message::Text(text) = ws.read().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

/// Connects like BenchUp does, retrying until the bridge listens.
fn connect_app(port: u16) -> WebSocket<TcpStream> {
    let started = Instant::now();
    let stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if started.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => panic!("the bridge did not start: {e}"),
        }
    };
    let (mut ws, _) = tungstenite::client::client(format!("ws://127.0.0.1:{port}/"), stream).unwrap();
    ws.send(Message::text(json!({"type": "authenticate", "ticket": "", "connection": "benchup-app-test"}).to_string())).unwrap();
    let challenge = read(&mut ws);
    let bridge_nonce = challenge["nonce"].as_str().unwrap().to_owned();
    let nonce = "cd".repeat(32);
    let proof = pairing::app_proof(TOKEN, &bridge_nonce, &nonce);
    ws.send(Message::text(json!({"type": "prove", "nonce": nonce, "proof": proof}).to_string())).unwrap();
    assert_eq!(read(&mut ws)["type"], "ready");
    ws
}

/// Answers calls the way the app does for the few tools used here.
fn serve_app(mut ws: WebSocket<TcpStream>) -> std::thread::JoinHandle<Vec<Value>> {
    std::thread::spawn(move || {
        let mut seen = Vec::new();
        while let Ok(message) = ws.read() {
            let Message::Text(text) = message else { continue };
            let call: Value = serde_json::from_str(&text).unwrap();
            if call["type"] != "call" {
                continue;
            }
            let result = match call["name"].as_str().unwrap() {
                "get_scene" => json!({"ok": true, "object_count": 2, "client": call["client"]}),
                // `single`: one call answers with its own result.
                "run_batch" if call["arguments"]["single"] == true => json!({"ok": true, "created": [7], "dry_run": true}),
                "run_batch" => json!({"ok": true, "results": [{"ok": true, "created": [7]}], "dry_run": call["arguments"]["dry_run"]}),
                "render_view" => json!({"ok": true, "png_base64": "iVBORw==", "info": {"view": call["arguments"]["view"]}}),
                _ => json!({"ok": false, "error": "denied by the designer"}),
            };
            let ok = result["ok"] == true;
            ws.send(Message::text(json!({"type": "result", "id": call["id"], "ok": ok, "result": result}).to_string())).unwrap();
            seen.push(call);
            if seen.len() == 4 {
                break;
            }
        }
        seen
    })
}

#[test]
fn live_commands_run_in_the_app_through_the_bridge() {
    let port = free_port();
    let _bridge = Bridge(benchup(port, &["bridge", "--no-mcp"]).stdin(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let app = serve_app(connect_app(port));

    let (scene, output) = run(port, &["scene", "--live"]);
    assert!(output.status.success(), "{scene}");
    assert_eq!(scene["object_count"], 2);
    assert_eq!(scene["client"], "benchup-cli");

    // A dry run of one call answers with that call's result.
    let (result, _) = run(port, &["call", "--live", "--dry-run", "create_board", r#"{"name":"$1"}"#]);
    assert_eq!(result["created"], json!([7]));
    assert_eq!(result["dry_run"], true);
    assert!(result.get("result").is_none(), "{result}");

    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("view.png");
    let (render, _) = run(port, &["render", "--live", "--view", "front", "-o", png.to_str().unwrap()]);
    assert_eq!(render["view"], "front");
    assert_eq!(std::fs::read(&png).unwrap(), b"\x89PNG");

    let (refused, output) = run(port, &["call", "--live", "erase", r#"{"ids":[1]}"#]);
    assert_eq!(output.status.code(), Some(1), "a refused call is a tool error");
    assert_eq!(refused["error"], "denied by the designer");

    let calls = app.join().unwrap();
    assert_eq!(calls[1]["name"], "run_batch");
    assert_eq!(calls[1]["arguments"]["calls"][0]["tool"], "create_board");
    // One call is run as written: "$1" is a name, not a reference.
    assert_eq!(calls[1]["arguments"]["single"], true);
    assert_eq!(calls[1]["arguments"]["calls"][0]["arguments"]["name"], "$1");
}

#[test]
fn live_commands_need_the_pairing_token() {
    let output = Command::new(env!("CARGO_BIN_EXE_benchup"))
        .args(["scene", "--live"])
        .env_remove("BENCHUP_AGENT_TOKEN")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("BENCHUP_AGENT_TOKEN"));
}
