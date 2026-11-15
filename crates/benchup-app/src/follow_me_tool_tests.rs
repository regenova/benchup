use super::*;

fn app() -> (BenchUpApp, u64, [u64; 2]) {
    let mut model = Model::default();
    let points = [
        DVec3::new(0., -0.5, -0.5),
        DVec3::new(0., 0.5, -0.5),
        DVec3::new(0., 0.5, 0.5),
        DVec3::new(0., -0.5, 0.5),
    ];
    for i in 0..4 {
        model.add_edge(points[i], points[(i + 1) % 4]).unwrap();
    }
    let profile = *model.faces.keys().next().unwrap();
    let edges = [
        model.add_edge(DVec3::ZERO, DVec3::X * 5.).unwrap(),
        model
            .add_edge(DVec3::X * 5., DVec3::new(5., 5., 0.))
            .unwrap(),
    ];
    let mut app = BenchUpApp::with_document(
        Document {
            model,
            ..Default::default()
        },
        None,
        true,
    );
    app.set_tool(Tool::FollowMe);
    (app, profile, edges)
}

#[test]
fn traced_preview_is_cached_backtracks_and_never_changes_the_document() {
    let (mut app, profile, edges) = app();
    let before = app.doc.model.clone();
    app.follow_me.profile = Some(profile);
    app.trace_follow_me_edge(edges[0]);
    assert_eq!(app.follow_me.preview_updates, 1);
    app.trace_follow_me_edge(edges[0]);
    assert_eq!(app.follow_me.preview_updates, 1);
    app.trace_follow_me_edge(edges[1]);
    assert_eq!(app.follow_me.edges, edges);
    assert!(app.follow_me.preview.is_some());
    app.trace_follow_me_edge(edges[0]);
    assert_eq!(app.follow_me.edges, vec![edges[0]]);
    assert_eq!(app.doc.model, before);
    assert!(!app.project.modified);
    assert!(app.doc.history.undo_label().is_none());
    app.follow_me.reset();
    assert!(app.follow_me.preview.is_none());
    assert!(app.follow_me.profile.is_none());
    assert_eq!(app.doc.model, before);
}

#[test]
fn committing_the_preview_records_one_undoable_operation_and_tool_switch_cancels() {
    let ctx = egui::Context::default();
    let (mut app, profile, edges) = app();
    let before = app.doc.model.clone();
    app.follow_me.preferences.roll = 30.;
    app.follow_me.profile = Some(profile);
    app.trace_follow_me_edge(edges[0]);
    app.trace_follow_me_edge(edges[1]);
    let preview = app.follow_me.preview.take().unwrap();
    let result = std::mem::take(&mut app.follow_me.result);
    app.commit_follow_me(preview.clone(), result, &ctx);
    assert_eq!(app.doc.model, preview);
    assert!(app.follow_me.preview.is_none());
    assert_eq!(app.follow_me.preferences.roll, 30.);
    assert_eq!(app.doc.history.undo_label(), Some(tr!("follow_me.tool")));
    app.undo(&ctx, false);
    assert_eq!(app.doc.model, before);
    app.undo(&ctx, true);
    assert_eq!(app.doc.model, preview);
    app.undo(&ctx, false);
    app.follow_me.profile = Some(profile);
    app.trace_follow_me_edge(edges[0]);
    app.set_tool(Tool::Select);
    assert!(app.follow_me.preview.is_none());
    assert_eq!(app.doc.model, before);
    assert_eq!(app.follow_me.preferences.roll, 30.);
}

#[test]
fn invalid_trace_retains_the_error_and_cannot_commit_a_stale_preview() {
    let (mut app, profile, edges) = app();
    let before = app.doc.model.clone();
    app.follow_me.profile = Some(profile);
    app.trace_follow_me_edge(edges[0]);
    assert!(app.follow_me.preview.is_some());
    // Invalid orientation input must discard the previous valid preview.
    app.follow_me.preferences.roll = f64::NAN;
    app.refresh_follow_me_preview();
    assert!(app.follow_me.preview.is_none());
    assert!(app.follow_me.error.is_some());
    assert_eq!(app.doc.model, before);
}

#[test]
fn sparse_pointer_events_include_intermediate_edges_and_keep_a_completed_loop() {
    let (mut app, profile, edges) = app();
    let third = app
        .doc
        .model
        .add_edge(DVec3::new(5., 5., 0.), DVec3::new(0., 5., 0.))
        .unwrap();
    let fourth = app
        .doc
        .model
        .add_edge(DVec3::new(0., 5., 0.), DVec3::ZERO)
        .unwrap();
    app.follow_me.profile = Some(profile);
    app.trace_follow_me_edge(edges[0]);
    // One event skipped the second edge entirely.
    app.trace_follow_me_edge(third);
    assert_eq!(app.follow_me.edges, vec![edges[0], edges[1], third]);
    assert!(app.follow_me.preview.is_some());
    app.trace_follow_me_edge(fourth);
    assert!(app.follow_me.preview.is_some());
    app.trace_follow_me_edge(edges[0]);
    assert_eq!(app.follow_me.edges.len(), 4);
}
