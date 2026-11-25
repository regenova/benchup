use super::*;
use crate::test_ui::ContextExt as _;

const VIEWPORT: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1200., 800.));

fn draw(ctx: &egui::Context, app: &mut BenchUpApp, time: f64) -> egui::FullOutput {
    ctx.run_test_ui(
        egui::RawInput {
            screen_rect: Some(VIEWPORT),
            time: Some(time),
            ..Default::default()
        },
        |ui| {
            app.tool_rail(ui, VIEWPORT);
        },
    )
}

fn frame(ctx: &egui::Context, app: &mut BenchUpApp, time: f64) -> Rect {
    let output = draw(ctx, app, time);
    fn find(shape: &egui::Shape, color: egui::Color32, found: &mut Vec<Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    find(shape, color, found);
                }
            }
            egui::Shape::Rect(shape) if shape.fill == color && shape.rect.width() == 64. => {
                found.push(shape.rect);
            }
            _ => {}
        }
    }
    let painter = ctx.layer_painter(egui::LayerId::background());
    let selected = Palette::for_painter(&painter).selected;
    let mut backgrounds = Vec::new();
    for shape in output.shapes {
        find(&shape.shape, selected, &mut backgrounds);
    }
    assert_eq!(backgrounds.len(), 1, "one shared selection background");
    backgrounds[0]
}

#[test]
fn selection_slides_and_settles_without_delaying_tool_activation() {
    let ctx = egui::Context::default();
    apply(&ctx);
    ctx.style_mut_of(ctx.theme(), |s| s.animation_time = 0.14);
    let mut app = BenchUpApp::with_document(Document::default(), None, true);
    draw(&ctx, &mut app, 0.);
    let start = frame(&ctx, &mut app, 0.);
    app.set_tool(Tool::PushPull);
    let first = frame(&ctx, &mut app, 0.01);
    assert_eq!(app.tool, Tool::PushPull);
    assert_eq!(first, start);
    let middle = frame(&ctx, &mut app, 0.08);
    let end = frame(&ctx, &mut app, 0.3);
    assert!(middle.left() > start.left() && middle.left() < end.left());
    assert_eq!(frame(&ctx, &mut app, 0.6), end);
    app.set_tool(Tool::Select);
    frame(&ctx, &mut app, 0.61);
    let returning = frame(&ctx, &mut app, 0.68);
    assert!(returning.left() > start.left() && returning.left() < end.left());
    // A rapid change continues from the current position.
    app.set_tool(Tool::Move);
    let changed = frame(&ctx, &mut app, 0.68);
    assert!(
        changed.min.distance(returning.min) < 0.01 && changed.max.distance(returning.max) < 0.01
    );
}

#[test]
fn zero_duration_selection_snaps_and_nested_tools_share_their_slot() {
    let ctx = egui::Context::default();
    apply(&ctx);
    ctx.style_mut_of(ctx.theme(), |s| s.animation_time = 0.);
    let mut app = BenchUpApp::with_document(Document::default(), None, true);
    draw(&ctx, &mut app, 0.);
    let start = frame(&ctx, &mut app, 0.);
    app.set_tool(Tool::PushPull);
    let end = frame(&ctx, &mut app, 0.01);
    assert!(end.left() > start.left());
    app.set_tool(Tool::Rectangle);
    let drawing = frame(&ctx, &mut app, 0.02);
    app.set_tool(Tool::Circle);
    assert_eq!(frame(&ctx, &mut app, 0.03), drawing);
    app.set_tool(Tool::Router);
    let wood = frame(&ctx, &mut app, 0.04);
    app.set_tool(Tool::WoodGlue);
    assert!(frame(&ctx, &mut app, 0.05).left() > wood.left());
}

#[test]
fn image_appears_after_text_and_shares_its_toolbar_slot() {
    let text = Tool::MEASUREMENT_MENU
        .iter()
        .position(|tool| *tool == Tool::Text)
        .unwrap();
    assert_eq!(Tool::MEASUREMENT_MENU[text + 1], Tool::Image);
    assert!(!Tool::DRAWING.contains(&Tool::Image));
    assert!(Tool::Image.is_drawing());
    assert!(!Tool::Image.is_measuring());
    let ctx = egui::Context::default();
    apply(&ctx);
    ctx.style_mut_of(ctx.theme(), |s| s.animation_time = 0.);
    let mut app = BenchUpApp::with_document(Document::default(), None, true);
    app.set_tool(Tool::Rectangle);
    app.set_tool(Tool::Text);
    draw(&ctx, &mut app, 0.);
    let text_slot = frame(&ctx, &mut app, 0.);
    app.set_tool(Tool::Image);
    assert_eq!(frame(&ctx, &mut app, 0.01), text_slot);
    assert_eq!(app.measurement_tool, Tool::Image);
    assert_eq!(app.drawing_tool, Tool::Rectangle);
}
