use super::*;
#[cfg(test)]
use crate::test_ui::ContextExt as _;
use eframe::Storage;

#[derive(Default)]
struct MemoryStorage {
    values: BTreeMap<String, String>,
    flushes: usize,
}
impl Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.values.insert(key.to_owned(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.values.remove(key);
    }
    fn flush(&mut self) {
        self.flushes += 1;
    }
}

struct UiHarness {
    app: BenchUpApp,
    ctx: egui::Context,
    time: f64,
}
impl UiHarness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        apply(&ctx);
        let mut harness = Self {
            app: BenchUpApp::with_document(Document::default(), None, true),
            ctx,
            time: 0.,
        };
        harness.frame(vec![]);
        harness.frame(vec![]);
        harness
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.01;
        self.ctx.run_test_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000., 700.))),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                self.app.viewport_panel(ui);
            },
        )
    }
    fn pointer(&mut self, pos: Pos2, button: egui::PointerButton, pressed: bool) {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    fn click(&mut self, pos: Pos2) {
        self.pointer(pos, egui::PointerButton::Primary, true);
        self.pointer(pos, egui::PointerButton::Primary, false);
    }
    fn label_in(&mut self, label: &str, rect: Rect) -> Pos2 {
        self.frame(vec![])
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text == label => {
                    let pos = text.pos + text.galley.size() * 0.5;
                    rect.contains(pos).then_some(pos)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"))
    }
}

#[test]
fn viewport_shows_only_the_four_view_buttons_and_keeps_the_model_breadcrumb() {
    let mut harness = UiHarness::new();
    let viewport = harness.app.viewport;
    let output = harness.frame(vec![]);
    let labels: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.job.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(labels.contains(&"Model"));
    for removed in ["Current", "+", "Custom views"] {
        assert!(!labels.contains(&removed));
    }
    let mut previous_x = viewport.left();
    for label in ["3D", "Front", "Side", "Top"] {
        let pos = harness.label_in(label, viewport);
        assert!(pos.x > previous_x);
        assert!(pos.y < viewport.top() + 40.);
        previous_x = pos.x;
    }
}

#[test]
fn quick_view_buttons_stay_opaque_during_an_orbit_drag() {
    let mut harness = UiHarness::new();
    harness.time += 1.;
    let viewport = harness.app.viewport;
    let start = viewport.center();
    let yaw = harness.app.doc.camera.yaw;
    harness.pointer(start, egui::PointerButton::Middle, true);
    let output = harness.frame(vec![egui::Event::PointerMoved(
        start + egui::vec2(40., 12.),
    )]);
    assert!(harness.app.navigating());
    assert_ne!(harness.app.doc.camera.yaw, yaw);

    for label in ["3D", "Front", "Side", "Top"] {
        let opacity = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.job.text == label && viewport.contains(text.pos) =>
            {
                text.galley
                    .rows
                    .iter()
                    .flat_map(|row| &row.visuals.mesh.vertices)
                    .map(|vertex| vertex.color.a())
                    .max()
            }
            _ => None,
        });
        assert_eq!(opacity, Some(255), "{label}");
    }
}

#[test]
fn quick_view_buttons_switch_projection_and_restore_through_preferences() {
    use benchup_core::{Projection, StandardView};

    let mut harness = UiHarness::new();
    let camera = harness.app.doc.camera.clone();
    let revision = harness.app.document_model.revision();
    let rect = harness.app.viewport;
    for (label, view) in [
        ("Front", StandardView::Front),
        ("Side", StandardView::Right),
        ("Top", StandardView::Top),
    ] {
        let button = harness.label_in(label, rect);
        harness.click(button);
        assert_eq!(harness.app.doc.camera.standard_view(), Some(view));
        assert_eq!(harness.app.doc.camera.projection, Projection::Parallel);
        assert_eq!(harness.app.doc.camera.target, camera.target);
        assert_eq!(harness.app.doc.camera.distance, camera.distance);
        assert!(harness.app.gesture.is_none());
        assert_eq!(harness.app.document_model.revision(), revision);
    }

    assert!(harness.app.view_transition.is_some());
    let mut storage = MemoryStorage::default();
    harness
        .app
        .persist_inspector_changes(&harness.ctx, Some(&mut storage), false);
    let mut restored = BenchUpApp::with_document(Document::default(), None, true);
    restored.restore_inspector_preferences(&egui::Context::default(), Some(&storage));
    assert_eq!(restored.doc.camera, harness.app.doc.camera);
    assert!(restored.view_transition.is_none());

    let button = harness.label_in("3D", rect);
    harness.click(button);
    assert_eq!(harness.app.doc.camera, camera);
}

#[test]
fn quick_view_motion_retargets_from_the_visible_camera_and_yields_to_canvas_input() {
    use super::quick_views::VIEW_TRANSITION_DURATION;

    let mut harness = UiHarness::new();
    let rect = harness.app.viewport;
    let front = harness.label_in("Front", rect);
    let top = harness.label_in("Top", rect);
    harness.click(front);
    let destination = harness.app.doc.camera.clone();
    harness.time += 0.07;
    harness.pointer(top, egui::PointerButton::Primary, true);
    let visible = harness
        .app
        .view_transition
        .as_ref()
        .unwrap()
        .sample(&destination, harness.time + 0.01)
        .unwrap();
    assert!(visible.projection_blend.is_some());
    harness.pointer(top, egui::PointerButton::Primary, false);
    let destination = harness.app.doc.camera.clone();
    assert_eq!(
        harness
            .app
            .view_transition
            .as_ref()
            .unwrap()
            .sample(&destination, harness.time),
        Some(visible)
    );

    harness.pointer(rect.center(), egui::PointerButton::Primary, true);
    assert!(harness.app.view_transition.is_none());
    assert!(harness.app.gesture.is_some());
    assert!(harness.app.doc.camera.projection_blend.is_none());
    harness.pointer(rect.center(), egui::PointerButton::Primary, false);

    harness.click(front);
    let destination = harness.app.doc.camera.clone();
    harness.time += VIEW_TRANSITION_DURATION;
    harness.frame(vec![]);
    assert!(harness.app.view_transition.is_none());
    assert_eq!(harness.app.doc.camera, destination);
}
