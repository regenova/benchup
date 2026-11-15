use super::*;

#[test]
fn follow_me_options_live_at_the_viewport_top_right_and_do_not_edit_geometry() {
    fn contains_text(output: &egui::FullOutput, label: &str) -> bool {
        fn find(shape: &egui::Shape, label: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text == label,
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| find(shape, label)),
                _ => false,
            }
        }
        output.shapes.iter().any(|shape| find(&shape.shape, label))
    }

    let mut h = Harness::new();
    h.size.y = 2000.;
    let output = h.frame(vec![], egui::Modifiers::NONE);
    let advanced = text_position(&output, tr!("follow_me.advanced_tools"));
    h.frame(
        vec![egui::Event::PointerMoved(advanced)],
        egui::Modifiers::NONE,
    );
    let tools = h.frame(vec![], egui::Modifiers::NONE);
    assert_menu_icon(&tools, Tool::FollowMe.label());
    assert!(!contains_text(&tools, tr!("follow_me.reverse")));
    h.click(menu_text_position(&tools, Tool::FollowMe.label()));
    assert_eq!(h.app.tool, Tool::FollowMe);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    h.frame(vec![egui::Event::PointerGone], egui::Modifiers::NONE);
    let options = h.frame(vec![], egui::Modifiers::NONE);
    let reverse = text_position(&options, tr!("follow_me.reverse"));
    assert!(reverse.x > h.app.viewport.center().x);
    assert!(reverse.y > h.app.viewport.top() && reverse.y < h.app.viewport.top() + 48.);

    let before = h.app.doc.model.clone();
    h.click(reverse);
    assert!(h.app.follow_me.preferences.reverse);
    assert_eq!(h.app.tool, Tool::FollowMe);
    assert!(
        h.app.gesture.is_none(),
        "overlay clicks must not reach the modeling tool"
    );
    assert_eq!(h.app.doc.model, before);
    assert!(!h.app.project.modified);
    assert!(h.app.doc.history.is_empty());

    h.app.set_tool(Tool::Select);
    let output = h.frame(vec![egui::Event::PointerGone], egui::Modifiers::NONE);
    assert!(!contains_text(&output, tr!("follow_me.reverse")));
    h.app.set_tool(Tool::FollowMe);
    h.frame(vec![], egui::Modifiers::NONE);
    assert!(h.app.follow_me.preferences.reverse);
}

fn fixture() -> (Harness, u64, [u64; 2]) {
    let mut h = Harness::new();
    let points = [
        DVec3::new(0., -0.5, -0.5),
        DVec3::new(0., 0.5, -0.5),
        DVec3::new(0., 0.5, 0.5),
        DVec3::new(0., -0.5, 0.5),
    ];
    for i in 0..4 {
        h.app
            .doc
            .model
            .add_edge(points[i], points[(i + 1) % 4])
            .unwrap();
    }
    let profile = *h.app.doc.model.faces.keys().next().unwrap();
    let edges = [
        h.app
            .doc
            .model
            .add_edge(DVec3::ZERO, DVec3::X * 5.)
            .unwrap(),
        h.app
            .doc
            .model
            .add_edge(DVec3::X * 5., DVec3::new(5., 5., 0.))
            .unwrap(),
    ];
    h.app.fit(&h.ctx);
    h.frame(vec![], egui::Modifiers::NONE);
    (h, profile, edges)
}

#[test]
fn preselect_path_activate_and_click_face_creates_one_sweep() {
    let (mut h, profile, edges) = fixture();
    let before = h.app.doc.model.clone();
    h.app.selected = edges.into_iter().collect();
    h.app.set_tool(Tool::FollowMe);
    h.click(h.at(DVec3::new(0., 0.1, 0.1)));
    assert!(!h.app.doc.model.faces.contains_key(&profile));
    assert_eq!(h.app.doc.history.undo_label(), Some(tr!("follow_me.tool")));
    assert!(h.app.follow_me.preview.is_none());
    h.app.undo(&h.ctx, false);
    assert_eq!(h.app.doc.model, before);
}

#[test]
fn manual_trace_previews_without_editing_and_escape_cancels() {
    let (mut h, _, _) = fixture();
    h.app.selected.clear();
    h.app.set_tool(Tool::FollowMe);
    let before = h.app.doc.model.clone();
    h.click(h.at(DVec3::new(0., 0.1, 0.1)));
    let first = h.at(DVec3::new(2.5, 0., 0.));
    h.frame(
        vec![egui::Event::PointerMoved(first)],
        egui::Modifiers::NONE,
    );
    assert!(h.app.follow_me.preview.is_some());
    let second = h.at(DVec3::new(5., 2.5, 0.));
    h.frame(
        vec![egui::Event::PointerMoved(second)],
        egui::Modifiers::NONE,
    );
    assert!(h.app.follow_me.preview.is_some());
    assert_eq!(h.app.doc.model, before);
    h.key(Key::Escape);
    assert!(h.app.follow_me.preview.is_none());
    assert_eq!(h.app.tool, Tool::FollowMe);
    assert_eq!(h.app.doc.model, before);
}
