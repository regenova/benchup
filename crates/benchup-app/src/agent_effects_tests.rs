use super::*;

#[test]
fn inserting_or_erasing_one_child_never_animates_the_surviving_assembly() {
    let mut before = Model::default();
    let mut children = BTreeSet::new();
    for i in 0..4096 {
        let edge = before
            .add_edge(DVec3::Y * i as f64, DVec3::Y * i as f64 + DVec3::X)
            .unwrap();
        children.insert(edge);
    }
    let owner = before.make_object(&children, false, "Assembly").unwrap();
    let mut after = before.clone();
    let added = after.add_edge(DVec3::Z, DVec3::Z + DVec3::X).unwrap();
    after
        .objects
        .get_mut(&owner)
        .unwrap()
        .entities
        .insert(added);
    let created = Change::between(&before, &after, &Default::default());
    assert_eq!(created.ids, BTreeSet::from([added]));
    assert!(
        created.ghost.is_none(),
        "creation must not ghost the old assembly"
    );
    let deleted = Change::between(&after, &before, &Default::default());
    assert_eq!(deleted.ids, BTreeSet::from([added]));
    assert_eq!(deleted.ghost.as_ref().unwrap().vertices.len(), 6);
}

#[test]
fn outliner_metadata_never_allocates_a_departing_snapshot() {
    let mut before = Model::default();
    let edge = before.add_edge(DVec3::ZERO, DVec3::X).unwrap();
    let owner = before
        .make_object(&BTreeSet::from([edge]), false, "Part")
        .unwrap();
    let mut after = before.clone();
    after.objects.get_mut(&owner).unwrap().name = "Renamed".into();
    after.objects.get_mut(&owner).unwrap().outliner_order = Some(100);
    let change = Change::between(&before, &after, &Default::default());
    assert!(change.ids.is_empty());
    assert!(change.ghost.is_none());
}

#[test]
fn rapid_edits_reuse_one_departing_snapshot_per_changed_region() {
    let mut before = Model::default();
    let edge = before.add_edge(DVec3::ZERO, DVec3::X).unwrap();
    let mut after = before.clone();
    let vertex = after.edges[&edge].a;
    after.vertices.get_mut(&vertex).unwrap().position.z = 1.;
    let mut state = State::default();
    for i in 0..16 {
        state.record(
            Change::between(&before, &after, &Default::default()),
            10. + i as f64 * 0.01,
        );
    }
    assert_eq!(state.ghosts.len(), 1, "no sixteen-layer overdraw trail");
}

#[test]
fn precision_meshes_inherit_timestamps_for_assemblies_and_machined_surfaces() {
    let mut model = Model::default();
    let edge = model.add_edge(DVec3::ZERO, DVec3::X).unwrap();
    let owner = model
        .make_object(&BTreeSet::from([edge]), false, "Part")
        .unwrap();
    let derived = model.add_edge(DVec3::Z, DVec3::Z + DVec3::X).unwrap();
    model
        .routing_appearance
        .by_source
        .insert(edge, BTreeSet::from([derived]));
    let mut frame = Frame {
        starts: Arc::new(BTreeMap::from([(owner, 10.), (edge, 10.2)])),
        ..Default::default()
    };
    frame.remap(&model);
    assert_eq!(frame.stamp(edge), timestamp(10.2));
    assert_eq!(frame.stamp(derived), timestamp(10.2));
    assert_eq!(frame.stamp(owner), timestamp(10.));
}

#[test]
fn changed_vertices_and_deleted_geometry_keep_independent_transition_times() {
    let mut before = Model::default();
    let edge = before.add_edge(DVec3::ZERO, DVec3::X).unwrap();
    let untouched = before.add_edge(DVec3::Y, DVec3::Y + DVec3::X).unwrap();
    let mut moved = before.clone();
    let vertex = moved.edges[&edge].a;
    moved.vertices.get_mut(&vertex).unwrap().position.z += 1.;
    let change = Change::between(&before, &moved, &Default::default());
    assert!(change.ids.contains(&edge));
    assert!(!change.ids.contains(&untouched));
    assert!(
        change.ghost.is_some(),
        "retain the old position for dissolution"
    );
    let mut state = State::default();
    state.record(change, 4095.8);
    let frame = state.frame(4096.1, None, before.bounds());
    assert!((frame.stamp(edge) - 4095.8).abs() < 0.001);
    assert_eq!(frame.stamp(untouched), -1.);
    assert!((timestamp(frame.now) - 0.1).abs() < 0.001);

    let mut erased = moved.clone();
    erased.erase(&BTreeSet::from([edge]));
    let change = Change::between(&moved, &erased, &Default::default());
    assert!(change.ghost.is_some(), "erase still has departing geometry");
    state.record(change, 4096.2);
    let frame = state.frame(4096.8, None, erased.bounds());
    assert_eq!(
        frame.ghosts.len(),
        1,
        "the earlier transition expires independently"
    );
    assert!((frame.stamp(edge) - 0.2).abs() < 0.001);
    assert!(!state.frame(4097.2, None, None).animated());
}

#[test]
fn scan_runs_up_down_up_without_scanning_or_projecting_the_model() {
    let mut camera = Camera::default();
    camera.fit(
        Some((DVec3::splat(-1.), DVec3::splat(1.))),
        DVec2::new(800., 600.),
    );
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
    let frame = Frame {
        now: 10.,
        activity: Some((Mode::Looking, 10.)),
        bounds: Some((DVec3::splat(-1.), DVec3::splat(1.))),
        ..Default::default()
    };
    let mut ys = Vec::new();
    for second in 0..4 {
        let frame = Frame {
            now: 10. + second as f64,
            ..frame.clone()
        };
        let parameters = frame.parameters(&camera, rect);
        ys.push(parameters[1]);
    }
    assert!(ys[0] > ys[1]);
    assert!((ys[0] - ys[2]).abs() < 0.001);
    assert!((ys[1] - ys[3]).abs() < 0.001);
    let preparing = Frame {
        activity: Some((Mode::Preparing, 10.)),
        ..frame
    };
    assert_eq!(preparing.parameters(&camera, rect)[1], -1.);
}

#[test]
fn unchanged_and_hidden_geometry_do_not_leave_departing_ghosts() {
    let mut model = Model::default();
    let edge = model.add_edge(DVec3::ZERO, DVec3::X).unwrap();
    let unchanged = Change::between(&model, &model, &Default::default());
    assert!(unchanged.ids.is_empty());
    assert!(unchanged.ghost.is_none());
    model.hidden.insert(edge);
    let mut after = model.clone();
    after.erase(&BTreeSet::from([edge]));
    assert!(
        Change::between(&model, &after, &Default::default())
            .ghost
            .is_none()
    );
}
