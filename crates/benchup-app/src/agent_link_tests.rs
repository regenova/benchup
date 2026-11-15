use super::*;
use std::sync::{Arc, Mutex};

const TOKEN: &str = "6f1b0c8e-0d1a-4a52-9b0e-agent-test";
const BRIDGE_NONCE: &str = "abababababababababababababababababababababababababababababababab";

/// What crossed the fake socket.
#[derive(Default)]
struct Wire {
    incoming: Vec<Result<Value, String>>,
    sent: Vec<Value>,
    connects: usize,
}
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Wire>>);
impl Fake {
    fn push(&self, frame: Value) {
        self.0.lock().unwrap().incoming.push(Ok(frame));
    }
    fn sent(&self) -> Vec<Value> {
        std::mem::take(&mut self.0.lock().unwrap().sent)
    }
}
struct FakeLink(Fake);
impl Link for FakeLink {
    fn events(&mut self) -> Vec<Result<Value, String>> {
        std::mem::take(&mut self.0.0.lock().unwrap().incoming)
    }
    fn send(&mut self, message: Value) -> Result<(), String> {
        self.0.0.lock().unwrap().sent.push(message);
        Ok(())
    }
}
impl Connector for Fake {
    fn connect(&mut self, _: &egui::Context, url: &str, _: &str) -> Box<dyn Link> {
        assert!(url.starts_with("ws://127.0.0.1:"), "only loopback: {url}");
        self.0.lock().unwrap().connects += 1;
        Box::new(FakeLink(self.clone()))
    }
}

struct Setup {
    app: BenchUpApp,
    ctx: egui::Context,
    wire: Fake,
    _dir: tempfile::TempDir,
    next: u64,
}

impl Setup {
    /// An app whose agent connection is on, with a fake bridge, before the handshake.
    fn new() -> Self {
        let wire = Fake::default();
        let dir = tempfile::tempdir().unwrap();
        let mut app = BenchUpApp::with_document(Document::default(), None, true);
        app.agent_link = AgentLink::with_connector(Box::new(wire.clone()));
        app.agent_link.token_slot = Some(dir.path().to_owned());
        app.agent_link.token = Some(TOKEN.into());
        app.agent_link.token_checked = true;
        app.general_settings.agent.enabled = true;
        Self { app, ctx: egui::Context::default(), wire, _dir: dir, next: 0 }
    }
    fn poll(&mut self) {
        self.app.poll_agent_link(&self.ctx);
    }
    /// Connected and paired.
    fn paired() -> Self {
        let mut setup = Self::new();
        setup.poll();
        setup.wire.push(json!({"type": "challenge", "nonce": BRIDGE_NONCE, "protocol": 2}));
        setup.poll();
        let prove = setup.wire.sent().pop().unwrap();
        assert_eq!(prove["type"], "prove");
        let app_nonce = prove["nonce"].as_str().unwrap();
        assert_eq!(prove["proof"], pairing::app_proof(TOKEN, BRIDGE_NONCE, app_nonce));
        let proof = pairing::bridge_proof(TOKEN, BRIDGE_NONCE, app_nonce);
        setup.wire.push(json!({"type": "ready", "proof": proof, "protocol": 2}));
        setup.poll();
        assert!(setup.app.agent_link.connected(), "{:?}", setup.app.agent_link.status);
        setup
    }
    /// Sends a call; returns its id.
    fn send(&mut self, name: &str, arguments: Value) -> String {
        self.next += 1;
        let id = format!("c{}", self.next);
        self.wire.push(json!({"type": "call", "id": id, "name": name, "arguments": arguments,
            "client": "claude-code", "deadline_ms": now_ms() + 60_000}));
        id
    }
    /// The result frame for `id`, if one was sent.
    fn answer(&self, id: &str) -> Option<Value> {
        self.wire.sent().into_iter().find(|frame| frame["type"] == "result" && frame["id"] == id)
    }
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let id = self.send(name, arguments);
        self.poll();
        self.answer(&id).unwrap_or_else(|| panic!("{name} was not answered"))
    }
    /// Polls until `id` is answered (renders finish on another thread).
    fn wait_for(&mut self, id: &str) -> Value {
        let started = std::time::Instant::now();
        loop {
            self.poll();
            if let Some(answer) = self.answer(id) {
                return answer;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "{id} was not answered");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    /// The open project is shared with this user.
    fn share(&mut self) {
        self.app.account.server = "https://sharing.example".into();
        self.app.account.server_reachable = true;
        self.app.doc.shared_project = Some(benchup_core::SharedProject {
            server: self.app.account.server.clone(),
            id: "a".repeat(48),
            revision: 1,
            server_updated_at: 100,
            file_updated_at: 100,
        });
    }
}

fn board(name: &str, x: f64) -> Value {
    json!({"name": name, "origin_mm": [x, 0, 0], "size_mm": [600, 300, 18]})
}

fn labels(app: &BenchUpApp) -> Vec<String> {
    app.doc.history.labels().map(str::to_owned).collect()
}

#[test]
fn calls_wait_for_a_bridge_that_proves_the_token() {
    let mut setup = Setup::new();
    setup.poll();
    assert_eq!(setup.app.agent_link.status, Status::Connecting);
    // A call before pairing is ignored, never run.
    let early = setup.send("create_board", board("Early", 0.));
    setup.wire.push(json!({"type": "challenge", "nonce": BRIDGE_NONCE, "protocol": 2}));
    setup.poll();
    assert!(setup.answer(&early).is_none());
    setup.wire.push(json!({"type": "ready", "proof": "00".repeat(32), "protocol": 2}));
    setup.poll();
    assert!(matches!(setup.app.agent_link.status, Status::Error(_)), "a bridge without the token is refused");
    assert!(setup.app.doc.model.objects.is_empty());
}

#[test]
fn turning_it_off_disconnects_and_nothing_connects_while_off() {
    let mut setup = Setup::new();
    setup.app.general_settings.agent.enabled = false;
    setup.poll();
    assert_eq!(setup.wire.0.lock().unwrap().connects, 0);
    assert_eq!(setup.app.agent_link.status, Status::Off);
}

#[test]
fn each_changing_call_is_one_undo_step_and_reads_add_none() {
    let mut setup = Setup::paired();
    let answer = setup.call("create_board", board("Side", 0.));
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["result"]["document"]["name"], setup.app.doc.name);
    let id = answer["result"]["created"][0].as_u64().unwrap();
    assert_eq!(labels(&setup.app), ["Agent: create_board"]);
    assert_eq!(setup.app.selected, BTreeSet::from([id]), "what the agent made is selected");

    let scene = setup.call("get_scene", json!({}));
    assert_eq!(scene["ok"], true);
    assert_eq!(scene["result"]["project"]["name"], setup.app.doc.name);
    assert_eq!(labels(&setup.app).len(), 1, "reading adds no undo step");
    assert_eq!(setup.app.agent_link.client_name(), Some("claude-code"));

    let undone = setup.call("undo", json!({}));
    assert_eq!(undone["result"]["undone"], "Agent: create_board", "{undone}");
    assert!(setup.app.doc.model.objects.is_empty());
}

#[test]
fn activity_includes_reads_and_returns_to_idle_without_another_call() {
    let mut setup = Setup::paired();
    assert!(setup.app.agent_link.activity_text(clock()).is_none());

    assert_eq!(setup.call("get_scene", json!({}))["ok"], true);
    let (_, at) = setup.app.agent_link.last_activity.unwrap();
    assert_eq!(setup.app.agent_link.activity_text(at), Some("AI: Reading the project…".into()));
    assert!(setup.app.agent_link.activity_text(at + ACTIVITY_LINGER).is_none());
    assert!(!setup.app.agent_link.editing(at), "reading does not take editing control");

    assert_eq!(setup.call("create_board", board("Side", 0.))["ok"], true);
    assert_eq!(setup.app.agent_link.activity_text(clock()), Some("AI: Creating geometry…".into()));
    // A failed request must not leave a misleading description of work.
    assert_eq!(setup.call("get_object", json!({"id": u64::MAX}))["ok"], false);
    assert!(setup.app.agent_link.activity_text(clock()).is_none());
}

#[test]
fn waiting_activity_clears_on_cancellation_denial_and_disconnect() {
    let mut setup = Setup::paired();
    setup.app.line_start = Some(DVec3::ZERO);
    let id = setup.send("create_board", board("Side", 0.));
    setup.poll();
    assert_eq!(setup.app.agent_link.activity_text(clock()), Some("AI: Waiting for access — Creating geometry".into()));
    setup.wire.push(json!({"type": "cancel", "id": id}));
    setup.poll();
    assert!(setup.app.agent_link.activity_text(clock()).is_none());

    setup.send("create_board", board("Side", 0.));
    setup.poll();
    setup.app.agent_link.deny();
    assert!(setup.app.agent_link.activity_text(clock()).is_none());

    assert_eq!(setup.call("get_scene", json!({}))["ok"], true, "reads still work after denying edits");
    assert!(setup.app.agent_link.activity_text(clock()).is_some());
    setup.wire.0.lock().unwrap().incoming.push(Err("disconnected".into()));
    setup.poll();
    assert!(setup.app.agent_link.activity_text(clock()).is_none());
    assert!(setup.app.agent_link.last_activity.is_none());
}

#[test]
fn single_call_batches_describe_the_action_and_previews_are_identified() {
    let mut setup = Setup::paired();
    let calls = json!([{"tool": "create_board", "arguments": board("Side", 0.)}]);
    assert_eq!(setup.call("run_batch", json!({"calls": calls, "dry_run": true}))["ok"], true);
    assert_eq!(setup.app.agent_link.activity_text(clock()), Some("AI: Previewing changes…".into()));
    assert!(setup.app.doc.model.objects.is_empty());

    assert_eq!(setup.call("run_batch", json!({"calls": calls}))["ok"], true);
    assert_eq!(setup.app.agent_link.activity_text(clock()), Some("AI: Creating geometry…".into()));
    setup.app.doc_epoch += 1;
    setup.poll();
    assert!(setup.app.agent_link.activity_text(clock()).is_none(), "activity belongs to its project");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn rendering_activity_stays_visible_until_the_render_finishes() {
    let mut setup = Setup::paired();
    // Hold a render open deterministically without running a GPU render.
    let (tx, rx) = std::sync::mpsc::channel();
    setup.app.agent_link.rendering = Some(("render".into(), rx, setup.app.doc_epoch));
    let later = clock() + ACTIVITY_LINGER * 10.;
    assert_eq!(setup.app.agent_link.activity_text(later), Some("AI: Rendering a view…".into()));
    // Concurrent read calls must not hide an in-progress render.
    assert_eq!(setup.call("get_scene", json!({}))["ok"], true);
    assert_eq!(setup.app.agent_link.activity_text(later), Some("AI: Rendering a view…".into()));
    tx.send((true, json!({"png_base64": "test"}))).unwrap();
    setup.poll();
    let (_, at) = setup.app.agent_link.last_activity.unwrap();
    assert!(setup.app.agent_link.rendering.is_none());
    assert_eq!(setup.app.agent_link.activity_text(at), Some("AI: Rendering a view…".into()));
    assert!(setup.app.agent_link.activity_text(at + ACTIVITY_LINGER).is_none());

    let (tx, rx) = std::sync::mpsc::channel();
    setup.app.agent_link.rendering = Some(("old-project-render".into(), rx, setup.app.doc_epoch));
    setup.app.doc_epoch += 1;
    setup.poll();
    assert!(setup.app.agent_link.activity_text(clock()).is_none(), "the previous project's render stays hidden");
    tx.send((true, json!({"png_base64": "test"}))).unwrap();
    setup.poll();
    assert!(setup.app.agent_link.activity_text(clock()).is_none(), "finishing an old render does not revive its activity");
}

#[test]
fn a_batch_is_one_step_all_or_nothing_and_a_dry_run_changes_nothing() {
    let mut setup = Setup::paired();
    let calls = json!([
        {"tool": "create_board", "arguments": board("Left", 0.)},
        {"tool": "create_board", "arguments": board("Right", 700.)},
        {"tool": "get_object", "arguments": {"id": "$1.created[0]"}},
    ]);
    let answer = setup.call("run_batch", json!({"calls": calls}));
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["result"]["results"].as_array().unwrap().len(), 3);
    assert_eq!(labels(&setup.app), ["Agent: create_board"]);
    assert_eq!(setup.app.doc.model.root_entities().len(), 2);

    let failing = json!([
        {"tool": "create_board", "arguments": board("Top", 0.)},
        {"tool": "move", "arguments": {"ids": [999_999], "delta_mm": [1, 0, 0]}},
    ]);
    let answer = setup.call("run_batch", json!({"calls": failing}));
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["result"]["failed_call"], 2);
    assert_eq!(setup.app.doc.model.root_entities().len(), 2, "nothing of a failed batch stays");

    let dry = json!([{"tool": "create_board", "arguments": board("Trial", 0.)}]);
    let answer = setup.call("run_batch", json!({"calls": dry, "dry_run": true}));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["result"]["dry_run"], true);
    assert_eq!(setup.app.doc.model.root_entities().len(), 2);
    assert_eq!(labels(&setup.app).len(), 1);
}

#[test]
fn the_designer_mid_operation_grants_access_and_their_operation_is_cancelled() {
    let mut setup = Setup::paired();
    setup.app.line_start = Some(DVec3::ZERO);
    let id = setup.send("create_board", board("Side", 0.));
    let scene = setup.send("get_scene", json!({}));
    setup.poll();
    assert!(setup.app.agent_link.asking, "the designer is asked first");
    let sent = setup.wire.sent();
    assert!(!sent.iter().any(|f| f["id"] == id));
    assert!(sent.iter().any(|f| f["id"] == scene), "the read queued behind it does not wait");
    assert!(setup.app.doc.model.objects.is_empty());

    setup.app.grant_agent_access(&setup.ctx);
    assert!(setup.app.line_start.is_none(), "granting cancels the designer's line");
    setup.poll();
    let answer = setup.answer(&id).unwrap();
    assert_eq!(answer["ok"], true);
    assert_eq!(setup.app.agent_link.control, Control::Agent);
    assert!(!setup.app.agent_link.asking);

    // With access granted, a new operation of the designer gives way.
    setup.app.line_start = Some(DVec3::ZERO);
    let answer = setup.call("create_board", board("Other", 700.));
    assert_eq!(answer["ok"], true);
    assert!(setup.app.line_start.is_none());
}

#[test]
fn reads_run_while_the_designer_works_and_a_denial_refuses_changes() {
    let mut setup = Setup::paired();
    setup.app.line_start = Some(DVec3::ZERO);
    let scene = setup.call("get_scene", json!({}));
    assert_eq!(scene["ok"], true, "reading never needs a grant");
    assert!(!setup.app.agent_link.asking);

    let id = setup.send("create_board", board("Side", 0.));
    setup.poll();
    assert!(setup.app.agent_link.asking);
    setup.app.agent_link.deny();
    let answer = setup.answer(&id).unwrap();
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["result"]["error"], "The designer denied access to the project");
    assert!(answer["result"]["hint"].is_string());
    assert!(setup.app.line_start.is_some(), "the designer keeps working");

    // While they keep working, further changes are refused without asking.
    let again = setup.call("create_board", board("Again", 0.));
    assert_eq!(again["ok"], false);
    assert!(!setup.app.agent_link.asking);

    // Once they finish, the agent may edit again.
    setup.app.line_start = None;
    let answer = setup.call("create_board", board("Later", 0.));
    assert_eq!(answer["ok"], true, "{answer}");
}

#[test]
fn taking_control_makes_changes_wait_until_access_is_granted_or_released() {
    let mut setup = Setup::paired();
    setup.call("create_board", board("Side", 0.));
    let now = clock();
    assert!(setup.app.agent_link.editing(now), "the status bar offers Take control");

    setup.app.agent_link.take_control();
    assert_eq!(setup.app.agent_link.control, Control::Designer);
    let id = setup.send("create_board", board("Next", 700.));
    setup.poll();
    assert!(setup.app.agent_link.asking, "changes wait for a grant while the designer has control");
    assert!(setup.answer(&id).is_none());

    setup.app.agent_link.release_control();
    setup.poll();
    assert_eq!(setup.answer(&id).unwrap()["ok"], true);
    assert_eq!(setup.app.agent_link.control, Control::Shared);
}

#[test]
fn shared_projects_without_edit_access_stay_read_only() {
    let mut setup = Setup::paired();
    setup.share();
    let answer = setup.call("create_board", board("Side", 0.));
    assert_eq!(answer["ok"], false, "{answer}");
    assert!(setup.app.doc.model.objects.is_empty());
    assert!(setup.app.doc.history.labels().next().is_none());
    assert_eq!(setup.call("get_scene", json!({}))["ok"], true, "reading is still allowed");
}

#[test]
fn a_call_queued_for_a_project_that_was_replaced_is_refused() {
    let mut setup = Setup::paired();
    setup.app.project.external_change = true; // a file operation blocks calls
    let id = setup.send("create_board", board("Side", 0.));
    setup.poll();
    assert!(setup.answer(&id).is_none(), "it waits while the project is blocked");
    setup.app.doc_epoch += 1;
    setup.app.project.external_change = false;
    setup.poll();
    let answer = setup.answer(&id).unwrap();
    assert_eq!(answer["result"]["error"], "The open project changed before this call ran");
    assert!(setup.app.doc.model.objects.is_empty());
}

#[test]
fn malformed_calls_are_answered_with_an_error() {
    let mut setup = Setup::paired();
    setup.wire.push(json!({"type": "call", "id": "x", "name": "move", "arguments": [1, 2]}));
    setup.poll();
    let answer = setup.answer("x").unwrap();
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["result"]["error"], "The arguments must be a JSON object");
    let nested = setup.call("run_batch", json!({"calls": [{"tool": "undo"}]}));
    assert_eq!(nested["ok"], false);
}

#[test]
fn rendered_views_come_back_as_png() {
    let mut setup = Setup::paired();
    setup.call("create_board", board("Side", 0.));
    let first = setup.send("render_view", json!({"view": "front", "width": 200, "height": 150}));
    let second = setup.send("render_view", json!({"view": "top", "width": 200, "height": 150}));
    let scene = setup.send("get_scene", json!({}));
    setup.poll();
    assert!(setup.answer(&scene).is_some(), "reads do not wait for a render");
    for id in [first, second] {
        let answer = setup.wait_for(&id);
        assert_eq!(answer["ok"], true, "{answer}");
        assert!(answer["result"]["png_base64"].as_str().unwrap().starts_with("iVBOR"));
    }
}

#[test]
fn reads_behind_a_change_waiting_for_a_grant_still_run() {
    let mut setup = Setup::paired();
    setup.app.line_start = Some(DVec3::ZERO);
    let change = setup.send("create_board", board("Side", 0.));
    let later = setup.send("create_board", board("Other", 700.));
    let scene = setup.send("get_scene", json!({}));
    setup.poll();
    let sent = setup.wire.sent();
    assert!(sent.iter().any(|f| f["id"] == scene && f["ok"] == true), "the read is not stuck behind the change");
    assert!(!sent.iter().any(|f| f["id"] == change || f["id"] == later));
    assert!(setup.app.agent_link.asking);

    setup.app.line_start = None;
    setup.poll();
    let sent = setup.wire.sent();
    assert!(sent.iter().any(|f| f["id"] == change && f["ok"] == true));
    assert!(sent.iter().any(|f| f["id"] == later && f["ok"] == true));
    assert_eq!(setup.app.doc.model.root_entities().len(), 2);
}

#[test]
fn an_edit_waits_for_a_shared_lease_on_its_way_instead_of_failing_as_read_only() {
    let mut setup = Setup::paired();
    let made = setup.call("create_board", board("Side", 0.));
    let board_id = made["result"]["created"][0].as_u64().unwrap();
    setup.share();
    let lease = BTreeSet::from([board_id]);
    setup.app.selected = BTreeSet::new();
    setup.app.simulate_shared_access("edit", lease.clone(), BTreeSet::new());
    let id = setup.send("move", json!({"ids": [board_id], "delta_mm": [10, 0, 0]}));
    setup.poll();
    assert!(setup.answer(&id).is_none(), "the edit waits for the server to confirm the lease");

    setup.app.selected = lease.clone();
    setup.app.simulate_shared_access("edit", lease.clone(), lease);
    let answer = setup.wait_for(&id);
    assert_eq!(answer["ok"], true, "{answer}");

    // A view-only share is read-only at once.
    setup.app.simulate_shared_access("view", BTreeSet::new(), BTreeSet::new());
    let answer = setup.call("create_board", board("Other", 700.));
    assert_eq!(answer["ok"], false);
    assert!(answer["result"]["error"].as_str().unwrap().contains("read-only"), "{answer}");
}

#[test]
fn an_agent_selection_in_a_shared_project_asks_the_server_for_its_lease() {
    let mut setup = Setup::paired();
    let a = setup.call("create_board", board("Left", 0.))["result"]["created"][0].as_u64().unwrap();
    let b = setup.call("create_board", board("Right", 700.))["result"]["created"][0].as_u64().unwrap();
    setup.share();
    let lease = BTreeSet::from([a, b]);
    setup.app.selected = lease.clone();
    setup.app.simulate_shared_access("edit", lease.clone(), lease);
    // Agent calls run in `App::logic`, before the frame snapshots the
    // selection, so the change must be reported to sharing explicitly;
    // otherwise later edits wait for a lease that is never requested.
    let answer = setup.call("select", json!({"ids": [a]}));
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(setup.app.selected, BTreeSet::from([a]));
    let (desired, dirty) = setup.app.requested_shared_selection();
    assert_eq!(desired, &BTreeSet::from([a]));
    assert!(dirty, "the lease request is sent to the server");
}

#[test]
fn a_result_too_large_for_the_bridge_becomes_an_error() {
    let mut setup = Setup::paired();
    setup.app.agent_link.max_result_bytes = 256;
    let answer = setup.call("create_board", board("Side", 0.));
    assert_eq!(answer["ok"], false);
    assert_eq!(answer["result"]["applied"], true, "the agent learns the change stayed: {answer}");
    assert!(answer["result"]["hint"].is_string());
    assert_eq!(setup.app.agent_link.queue.len(), 0);
    assert!(setup.app.agent_link.connected(), "the connection stays up");
}

#[test]
fn bridge_refusals_are_shown_in_the_designers_language() {
    let mut setup = Setup::new();
    setup.poll();
    setup.wire.push(json!({"type": "error", "code": pairing::refusal::APP_BUSY, "message": "another window"}));
    setup.poll();
    assert_eq!(setup.app.agent_link.status, Status::Error(tr!("agent_link.error.app_busy").to_owned()));
}

#[test]
fn changing_calls_are_recognized() {
    assert!(changes_model("create_board", &json!({})));
    assert!(!changes_model("get_scene", &json!({})));
    assert!(changes_model("undo", &json!({})));
    assert!(!changes_model("render_view", &json!({})));
    let batch = json!({"calls": [{"tool": "get_scene"}, {"tool": "move"}]});
    assert!(changes_model("run_batch", &batch));
    assert!(!changes_model("run_batch", &json!({"calls": [{"tool": "get_scene"}]})));
    let mut dry = batch.clone();
    dry["dry_run"] = json!(true);
    assert!(!changes_model("run_batch", &dry));

    // Selecting changes no model but still needs a grant.
    assert!(!changes_model("select", &json!({"ids": [1]})));
    assert!(needs_grant("select", &json!({"ids": [1]})));
    let selecting = json!({"calls": [{"tool": "get_scene"}, {"tool": "select", "arguments": {"ids": [1]}}]});
    assert!(needs_grant("run_batch", &selecting));
    let mut dry = selecting.clone();
    dry["dry_run"] = json!(true);
    assert!(!needs_grant("run_batch", &dry), "a dry run leaves the selection alone");
    assert!(!needs_grant("get_selection", &json!({})));
}

#[test]
fn a_selection_waits_while_the_designer_is_mid_operation() {
    let mut setup = Setup::paired();
    let made = setup.call("create_board", board("Side", 0.));
    let id = made["result"]["created"][0].as_u64().unwrap();
    setup.app.selected.clear();
    setup.app.line_start = Some(DVec3::ZERO);
    let select = setup.send("select", json!({"ids": [id]}));
    let batch = setup.send("run_batch", json!({"calls": [{"tool": "select", "arguments": {"ids": [id]}}]}));
    setup.poll();
    assert!(setup.app.agent_link.asking, "the designer is asked first");
    assert!(setup.answer(&select).is_none());
    assert!(setup.app.selected.is_empty(), "the selection under their operation is untouched");

    setup.app.line_start = None;
    setup.poll();
    let sent = setup.wire.sent();
    assert!(sent.iter().any(|f| f["id"] == select && f["ok"] == true));
    assert!(sent.iter().any(|f| f["id"] == batch && f["ok"] == true));
    assert_eq!(setup.app.selected, BTreeSet::from([id]));
}

#[test]
fn a_finished_move_copy_does_not_keep_the_designer_busy() {
    let mut setup = Setup::paired();
    let model = setup.app.doc.model.clone();
    setup.app.last_move_copy = Some(MoveCopy { base: model.clone(), source: model, delta: DVec3::X });
    assert!(!setup.app.user_operation_active(), "an idle designer after a Move-copy is not busy");
    let answer = setup.call("create_board", board("Side", 0.));
    assert_eq!(answer["ok"], true, "{answer}");
    assert!(setup.app.last_move_copy.is_none(), "the agent's edit ends the array offer, like any edit");

    // Typing an array count is an operation.
    let model = setup.app.doc.model.clone();
    setup.app.last_move_copy = Some(MoveCopy { base: model.clone(), source: model, delta: DVec3::X });
    setup.app.length_input = "x3".into();
    assert!(setup.app.user_operation_active());
}

#[test]
fn the_dialog_closes_when_queued_changes_no_longer_need_a_grant() {
    let mut setup = Setup::paired();
    setup.app.line_start = Some(DVec3::ZERO);
    let ids: Vec<String> =
        (0..CALLS_PER_FRAME + 4).map(|i| setup.send("create_board", board(&format!("B{i}"), i as f64 * 700.))).collect();
    setup.poll();
    assert!(setup.app.agent_link.asking);

    // The designer presses Escape: the changes run without a grant, over
    // several frames, and the dialog goes away at once.
    setup.app.line_start = None;
    setup.poll();
    assert!(setup.app.agent_link.waiting_changes() > 0, "some calls wait for the next frame");
    assert!(!setup.app.agent_link.asking, "no grant is needed any more");
    setup.poll();
    let sent = setup.wire.sent();
    for id in &ids {
        assert!(sent.iter().any(|f| f["id"] == *id && f["ok"] == true), "{id} ran");
    }
}

#[test]
fn setup_snippets_keep_the_token_out_of_program_arguments() {
    assert_eq!(claude_command("T", DEFAULT_PORT), "claude mcp add benchup -e BENCHUP_AGENT_TOKEN=T -- benchup bridge");
    assert!(claude_command("T", 5000).ends_with("benchup bridge --port 5000"));
    let config: Value = serde_json::from_str(&mcp_json("T", DEFAULT_PORT)).unwrap();
    assert_eq!(config["mcpServers"]["benchup"]["args"], json!(["bridge"]));
    assert_eq!(config["mcpServers"]["benchup"]["env"]["BENCHUP_AGENT_TOKEN"], "T");
    assert!(shell_setup("T", 5000).contains("BENCHUP_BRIDGE_PORT=5000"));
    assert_eq!(ui::masked("abcd-1234").chars().rev().take(4).collect::<String>(), "4321");
}

#[test]
fn preferences_default_off_and_older_settings_load() {
    let settings: settings::GeneralSettings = serde_json::from_str(r#"{"show_fps": true}"#).unwrap();
    assert_eq!(settings.agent, Preferences { enabled: false, port: DEFAULT_PORT });
    let mut saved = settings::GeneralSettings::default();
    saved.agent = Preferences { enabled: true, port: 5000 };
    let restored: settings::GeneralSettings = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert_eq!(restored.agent, saved.agent);
    let mut low = Preferences { enabled: true, port: 80 };
    low.normalize();
    assert_eq!(low.port, DEFAULT_PORT);
}

#[test]
fn the_token_is_made_once_and_kept_on_this_device() {
    let dir = tempfile::tempdir().unwrap();
    let mut link = AgentLink::with_connector(Box::new(Fake::default()));
    link.token_slot = Some(dir.path().to_owned());
    let token = link.ensure_token(0.).unwrap();
    assert_eq!(token.len(), 36);
    let mut again = AgentLink::with_connector(Box::new(Fake::default()));
    again.token_slot = Some(dir.path().to_owned());
    again.load_token();
    assert_eq!(again.token.as_deref(), Some(token.as_str()));
    again.regenerate();
    assert_ne!(again.token.as_deref(), Some(token.as_str()));

    let mut broken = AgentLink::with_connector(Box::new(Fake::default()));
    broken.token_slot = Some(dir.path().join("empty"));
    broken.generator = || Err("no randomness".into());
    assert!(broken.ensure_token(0.).is_none(), "no weak token is ever made");
}

#[test]
fn a_token_that_cannot_be_made_is_not_retried_every_frame() {
    static TRIES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = tempfile::tempdir().unwrap();
    let mut link = AgentLink::with_connector(Box::new(Fake::default()));
    link.token_slot = Some(dir.path().to_owned());
    link.generator = || {
        TRIES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err("no randomness".into())
    };
    for frame in 0..100 {
        assert!(link.ensure_token(f64::from(frame) / 60.).is_none());
    }
    assert_eq!(TRIES.load(std::sync::atomic::Ordering::SeqCst), 1);
    link.ensure_token(TOKEN_RETRY + 1.);
    assert_eq!(TRIES.load(std::sync::atomic::Ordering::SeqCst), 2, "it tries again later");
}

#[test]
fn a_token_that_cannot_be_saved_is_not_used() {
    let dir = tempfile::tempdir().unwrap();
    let mut link = AgentLink::with_connector(Box::new(Fake::default()));
    link.token_slot = Some(dir.path().to_owned());
    let saved = link.ensure_token(0.).unwrap();

    // Storage becomes unwritable (a file where the directory should be).
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, "").unwrap();
    link.token_slot = Some(blocked.join("slot"));
    link.regenerate();
    assert!(link.token_error.is_some());
    assert_eq!(link.token.as_deref(), Some(saved.as_str()), "the saved token stays in use");

    // With nothing saved yet, no unsaved token is handed out.
    let mut fresh = AgentLink::with_connector(Box::new(Fake::default()));
    fresh.token_slot = Some(blocked.join("slot"));
    assert!(fresh.ensure_token(0.).is_none());
    assert!(fresh.token_error.is_some());
}
