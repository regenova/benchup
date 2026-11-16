use super::*;
use crate::{History, ObjectKind};

fn profile(model: &mut Model, half_width: f64, half_height: f64, hole: bool) -> u64 {
    let mut ring = |w: f64, h: f64| {
        [
            DVec3::new(0., -w, -h),
            DVec3::new(0., w, -h),
            DVec3::new(0., w, h),
            DVec3::new(0., -w, h),
        ]
        .map(|p| model.raw_vertex(p))
        .to_vec()
    };
    let outer = ring(half_width, half_height);
    let holes = if hole {
        vec![ring(half_width * 0.4, half_height * 0.4)]
    } else {
        vec![]
    };
    model.raw_face(outer, holes, Some([120, 170, 200]))
}

fn path(model: &mut Model, points: &[DVec3], closed: bool) -> BTreeSet<u64> {
    let ids: Vec<_> = points.iter().map(|p| model.raw_vertex(*p)).collect();
    (0..ids.len() - usize::from(!closed))
        .map(|i| model.insert_edge(ids[i], ids[(i + 1) % ids.len()]))
        .collect()
}

fn watertight(model: &Model, result: &BTreeSet<u64>) {
    let mut uses: BTreeMap<(u64, u64), (usize, i32)> = BTreeMap::new();
    for id in result {
        let face = &model.faces[id];
        for ring in std::iter::once(&face.vertices).chain(&face.holes) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let entry = uses.entry((a.min(b), a.max(b))).or_default();
                entry.0 += 1;
                entry.1 += if a < b { 1 } else { -1 };
            }
        }
    }
    assert!(
        uses.values().all(|uses| *uses == (2, 0)),
        "each seam has two oppositely wound faces: {uses:?}"
    );
    model.validate().unwrap();
}

#[test]
fn straight_sweep_is_colored_watertight_and_one_undoable_edit() {
    let mut model = Model::default();
    let face = profile(&mut model, 1., 1., false);
    let edges = path(&mut model, &[DVec3::ZERO, DVec3::X * 10.], false);
    let before = model.clone();
    let result = model.follow_me(face, &edges, 0., false).unwrap();
    assert_eq!(result.len(), 6);
    watertight(&model, &result);
    assert!((model.solid_volume(*result.first().unwrap()).unwrap() - 40.).abs() < 1e-6);
    assert!(
        result
            .iter()
            .all(|id| model.face_colors[id] == [120, 170, 200])
    );
    assert!(edges.iter().all(|id| model.edges[id] == before.edges[id]));
    let mut history = History::default();
    history.record("Follow Me", before.clone(), &model);
    let after = model.clone();
    assert!(history.undo(&mut model));
    assert_eq!(model, before);
    assert!(history.redo(&mut model));
    assert_eq!(model, after);
}

#[test]
fn corners_curves_and_closed_loops_share_clean_seams() {
    let cases = [
        (
            vec![
                DVec3::ZERO,
                DVec3::new(10., 0., 0.),
                DVec3::new(10., 10., 0.),
            ],
            false,
        ),
        (
            vec![
                DVec3::ZERO,
                DVec3::new(10., 0., 0.),
                DVec3::new(10., 10., 0.),
                DVec3::new(0., 10., 0.),
            ],
            true,
        ),
        (
            (0..=24)
                .map(|i| {
                    let t = i as f64 * std::f64::consts::FRAC_PI_2 / 24.;
                    DVec3::new(10. * t.sin(), 10. * (1. - t.cos()), 0.)
                })
                .collect(),
            false,
        ),
        // Noncoplanar closed path exercises transport and closure correction.
        (
            vec![
                DVec3::ZERO,
                DVec3::new(10., 0., 0.),
                DVec3::new(12., 10., 4.),
                DVec3::new(0., 12., 0.),
            ],
            true,
        ),
    ];
    for (points, closed) in cases {
        let mut model = Model::default();
        let face = profile(&mut model, 0.2, 0.3, false);
        let edges = path(&mut model, &points, closed);
        let result = model.follow_me(face, &edges, 0., false).unwrap();
        watertight(&model, &result);
        assert!(!model.faces.contains_key(&face));
        let caps = result
            .iter()
            .filter(|id| {
                model.faces[id].vertices.len() == 4
                    && crate::faces::plane(&model, &model.faces[id].vertices)
                        .unwrap()
                        .normal
                        .x
                        .abs()
                        > 0.99999
            })
            .count();
        if closed && points.len() == 4 && points.iter().all(|p| p.z == 0.) {
            assert_eq!(
                result.len(),
                16,
                "closed path has walls and no internal caps"
            );
        }
        if !closed && points.len() == 2 {
            assert_eq!(caps, 2);
        }
    }
}

#[test]
fn hollow_profiles_keep_both_caps_and_inner_walls() {
    let mut model = Model::default();
    let face = profile(&mut model, 1., 1., true);
    let edges = path(&mut model, &[DVec3::ZERO, DVec3::X * 10.], false);
    let result = model.follow_me(face, &edges, 0., false).unwrap();
    watertight(&model, &result);
    assert_eq!(result.len(), 10);
    assert_eq!(
        result
            .iter()
            .filter(|id| model.faces[id].holes.len() == 1)
            .count(),
        2
    );
    assert!((model.solid_volume(*result.first().unwrap()).unwrap() - 33.6).abs() < 1e-6);
}

#[test]
fn concave_profiles_preserve_their_shape_and_offset_from_the_path() {
    let mut model = Model::default();
    let ring = [(0., 0.), (2., 0.), (2., 1.), (1., 1.), (1., 2.), (0., 2.)]
        .map(|(y, z)| model.raw_vertex(DVec3::new(0., y + 3., z + 4.)))
        .to_vec();
    let face = model.raw_face(ring, vec![], None);
    let edges = path(&mut model, &[DVec3::ZERO, DVec3::X * 10.], false);
    let result = model.follow_me(face, &edges, 0., false).unwrap();
    watertight(&model, &result);
    let (min, max) = model.selection_bounds(&result).unwrap();
    assert_eq!(min, DVec3::new(0., 3., 4.));
    assert_eq!(max, DVec3::new(10., 5., 6.));
    assert!((model.solid_volume(*result.first().unwrap()).unwrap() - 30.).abs() < 1e-6);
}

#[test]
fn an_attached_end_face_extends_the_existing_solid_without_an_internal_cap() {
    let mut model = Model::default();
    let start = profile(&mut model, 1., 1., false);
    let first = path(&mut model, &[DVec3::ZERO, -DVec3::X * 5.], false);
    let body = model.follow_me(start, &first, 0., false).unwrap();
    let cap = *body
        .iter()
        .find(|id| {
            model.faces[id]
                .vertices
                .iter()
                .all(|v| model.vertices[v].position.x == -5.)
        })
        .unwrap();
    // Extend the far end outward, from x=-5 to x=-10.
    let edges = path(&mut model, &[-DVec3::X * 5., -DVec3::X * 10.], false);
    model.follow_me(cap, &edges, 0., false).unwrap();
    let faces = model.faces.keys().copied().collect();
    watertight(&model, &faces);
    assert!(
        (model
            .solid_volume(*model.faces.keys().next().unwrap())
            .unwrap()
            - 40.)
            .abs()
            < 1e-6
    );
}

#[test]
fn roll_reverse_and_oblique_projection_are_precise() {
    for reverse in [false, true] {
        let mut model = Model::default();
        let face = profile(&mut model, 2., 1., false);
        let edges = path(&mut model, &[DVec3::ZERO, DVec3::X * 10.], false);
        let result = model.follow_me(face, &edges, 90., reverse).unwrap();
        watertight(&model, &result);
        let (_, max) = model.selection_bounds(&result).unwrap();
        assert!((max.y - 1.).abs() < 1e-6);
        assert!((max.z - 2.).abs() < 1e-6);
    }
    let mut model = Model::default();
    let face = profile(&mut model, 1., 1., false);
    let edges = path(&mut model, &[DVec3::ZERO, DVec3::new(10., 2., 0.)], false);
    let result = model.follow_me(face, &edges, 0., false).unwrap();
    watertight(&model, &result);
    let tangent = DVec3::new(10., 2., 0.).normalize();
    let cap = result
        .iter()
        .find(|id| {
            crate::faces::plane(&model, &model.faces[id].vertices)
                .unwrap()
                .normal
                .dot(tangent)
                .abs()
                > 0.999999
        })
        .unwrap();
    assert!((model.face_area(&model.faces[cap]) - 4. * tangent.x).abs() < 1e-6);
}

#[test]
fn invalid_paths_folds_and_locked_profiles_are_atomic() {
    let cases = [
        vec![DVec3::ZERO, DVec3::X * 10., DVec3::ZERO],
        vec![DVec3::ZERO, DVec3::Y * 10.],
        vec![DVec3::ZERO, DVec3::X * 0.1, DVec3::new(0.1, 10., 0.)],
        // Far-apart portions crossing through the swept surface.
        vec![
            DVec3::ZERO,
            DVec3::new(10., 0., 0.),
            DVec3::new(10., 10., 0.),
            DVec3::new(5., 10., 0.),
            DVec3::new(5., -10., 0.),
        ],
    ];
    for points in cases {
        let mut model = Model::default();
        let face = profile(&mut model, 1., 1., false);
        let edges = path(&mut model, &points, false);
        let before = model.clone();
        assert!(model.follow_me(face, &edges, 0., false).is_err());
        assert_eq!(model, before);
    }
    for kind in 0..4 {
        let mut model = Model::default();
        let face = profile(&mut model, 1., 1., false);
        let mut edges = path(&mut model, &[DVec3::ZERO, DVec3::X * 10.], false);
        match kind {
            0 => {
                model.locked.insert(face);
            }
            1 => {
                edges.extend(path(&mut model, &[DVec3::Y * 20., DVec3::Y * 30.], false));
            }
            2 => {
                let e = model.edges[edges.first().unwrap()].clone();
                let c = model.raw_vertex(DVec3::new(20., 10., 0.));
                let d = model.raw_vertex(DVec3::new(20., -10., 0.));
                edges.insert(model.insert_edge(e.b, c));
                edges.insert(model.insert_edge(e.b, d));
            }
            _ => {
                model.locked.insert(*edges.first().unwrap());
            }
        }
        let before = model.clone();
        assert!(model.follow_me(face, &edges, 0., false).is_err());
        assert_eq!(model, before);
    }
}

#[test]
fn preselected_face_uses_its_perimeter_and_scoped_edits_stay_in_the_group() {
    let mut model = Model::default();
    profile(&mut model, 0.2, 0.3, false);
    let perimeter = path(
        &mut model,
        &[
            DVec3::ZERO,
            DVec3::X * 10.,
            DVec3::new(10., 10., 0.),
            DVec3::Y * 10.,
        ],
        true,
    );
    let mut vertices = vec![model.edges[perimeter.first().unwrap()].a];
    // Use the same ordered perimeter points to fill its face.
    for p in [DVec3::X * 10., DVec3::new(10., 10., 0.), DVec3::Y * 10.] {
        vertices.push(
            model
                .vertices
                .values()
                .find(|v| v.position == p)
                .unwrap()
                .id,
        );
    }
    let path_face = model.raw_face(vertices, vec![], None);
    assert_eq!(
        model.follow_me_edges(&BTreeSet::from([path_face])).unwrap(),
        perimeter
    );
    let group = model
        .make_object(&model.root_entities(), false, "Sweep")
        .unwrap();
    assert!(matches!(model.objects[&group].kind, ObjectKind::Group));
    assert!(model.enter_object(group));
    // Grouping copies geometry and assigns new IDs. Resolve both faces from
    // the group's contents instead of using their pre-grouping IDs.
    let face = *model.objects[&group]
        .entities
        .iter()
        .find(|id| model.face_colors.get(id) == Some(&[120, 170, 200]))
        .unwrap();
    let path_face = *model.objects[&group]
        .entities
        .iter()
        .find(|id| **id != face && model.faces.contains_key(id))
        .unwrap();
    let path_before = model.faces[&path_face].clone();
    let perimeter = model.follow_me_edges(&BTreeSet::from([path_face])).unwrap();
    let result = model
        .follow_me(face, &BTreeSet::from([path_face]), 0., false)
        .unwrap();
    assert!(!result.is_empty());
    assert!(result.iter().all(|id| model.parent(*id) == Some(group)));
    assert!(!model.faces.contains_key(&face));
    assert_eq!(model.edit_context, Some(group));
    assert_eq!(model.faces[&path_face], path_before);
    assert_eq!(
        model.follow_me_edges(&BTreeSet::from([path_face])).unwrap(),
        perimeter
    );
    watertight(&model, &result);
}
