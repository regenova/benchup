//! Atomic profile sweeps. The CPU constructs authoritative topology only when
//! the traced path changes; the app retains and renders the result on the GPU.
use crate::{DVec3, Model, document::EPS, faces};
use glam::DQuat;
use std::collections::{BTreeMap, BTreeSet};

fn invalid_path() -> &'static str {
    tr!("follow_me.invalid_path")
}
fn invalid_sweep() -> &'static str {
    tr!("follow_me.invalid_sweep")
}

impl Model {
    /// Accept a chain of edges or a single face's outside perimeter, as in
    /// SketchUp's preselected-path workflow. Never expand groups implicitly.
    pub fn follow_me_edges(&self, selected: &BTreeSet<u64>) -> Result<BTreeSet<u64>, &'static str> {
        if selected.iter().any(|id| {
            self.parent(*id) != self.edit_context || self.is_hidden(*id) || self.is_locked(*id)
        }) {
            return Err(invalid_path());
        }
        let edges = if selected.len() == 1
            && let Some(face) = selected.first().and_then(|id| self.faces.get(id))
        {
            if face.vertices.len() > 4096 {
                return Err(tr!("follow_me.too_large"));
            }
            (0..face.vertices.len())
                .map(|i| {
                    let a = face.vertices[i];
                    let b = face.vertices[(i + 1) % face.vertices.len()];
                    self.edges
                        .values()
                        .find(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a))
                        .map(|e| e.id)
                        .ok_or_else(invalid_path)
                })
                .collect::<Result<BTreeSet<_>, _>>()?
        } else {
            selected.clone()
        };
        if edges.is_empty()
            || edges.iter().any(|id| {
                !self.edges.contains_key(id)
                    || self.parent(*id) != self.edit_context
                    || self.is_hidden(*id)
                    || self.is_locked(*id)
            })
        {
            return Err(invalid_path());
        }
        Ok(edges)
    }

    /// Sweep a planar face along a connected, unbranched path. Initial profile
    /// placement supplies its offset and orientation. Project onto the plane
    /// perpendicular to the first segment, then parallel-transport that frame.
    /// Positive roll is right-handed around the first segment, in degrees.
    pub fn follow_me(
        &mut self,
        profile: u64,
        path: &BTreeSet<u64>,
        roll_degrees: f64,
        reverse: bool,
    ) -> Result<BTreeSet<u64>, &'static str> {
        if self.edit_context.is_some() {
            return self
                .edit_in_context(|model| model.follow_me(profile, path, roll_degrees, reverse));
        }
        self.editable_surface(profile)?;
        let face = self
            .faces
            .get(&profile)
            .ok_or(tr!("follow_me.pick_profile"))?
            .clone();
        let plane = faces::plane(self, &face.vertices).ok_or(tr!("follow_me.pick_profile"))?;
        if !roll_degrees.is_finite() {
            return Err(invalid_sweep());
        }
        let path = self.follow_me_edges(path)?;
        if path.len() > 4096 {
            return Err(tr!("follow_me.too_large"));
        }
        let boundary: BTreeSet<_> = std::iter::once(&face.vertices)
            .chain(&face.holes)
            .flat_map(|r| {
                (0..r.len()).map(move |i| {
                    (
                        r[i].min(r[(i + 1) % r.len()]),
                        r[i].max(r[(i + 1) % r.len()]),
                    )
                })
            })
            .collect();
        if path.iter().any(|id| {
            let e = &self.edges[id];
            boundary.contains(&(e.a.min(e.b), e.a.max(e.b)))
        }) {
            return Err(invalid_path());
        }
        let center = face
            .vertices
            .iter()
            .map(|id| self.vertices[id].position)
            .sum::<DVec3>()
            / face.vertices.len() as f64;
        if self
            .edges
            .values()
            .any(|e| boundary.contains(&(e.a.min(e.b), e.a.max(e.b))) && self.is_locked(e.id))
        {
            return Err(tr!(
                "core.modeling.unlock_the_connected_geometry_before_editing_it"
            ));
        }
        let (points, closed) = ordered_path(self, &path, center, plane.normal, reverse)?;
        let segments = if closed {
            points.len()
        } else {
            points.len() - 1
        };
        let count: usize = face.vertices.len() + face.holes.iter().map(Vec::len).sum::<usize>();
        // Bound both interactive topology work and allocations before cloning.
        if count.saturating_mul(points.len()) > 100_000 {
            return Err(tr!("follow_me.too_large"));
        }
        let directions: Vec<_> = (0..segments)
            .map(|i| (points[(i + 1) % points.len()] - points[i]).normalize())
            .collect();
        let first = directions[0];
        if plane.normal.dot(first).abs() < 1e-5 {
            return Err(tr!("follow_me.parallel_profile"));
        }
        let roll = DQuat::from_axis_angle(first, roll_degrees.to_radians());
        let mut source_rings = vec![face.vertices.clone()];
        source_rings.extend(face.holes.clone());
        for (hole, ring) in source_rings.iter_mut().enumerate() {
            faces::plane(self, ring).ok_or_else(invalid_sweep)?;
            // plane() uses the first noncollinear corner, which can be concave.
            let area = (0..ring.len())
                .map(|i| {
                    let a = self.vertices[&ring[i]].position - plane.origin;
                    let b = self.vertices[&ring[(i + 1) % ring.len()]].position - plane.origin;
                    a.cross(b).dot(first)
                })
                .sum::<f64>();
            if (area > 0.) != (hole == 0) {
                ring.reverse();
            }
        }
        let offsets: Vec<Vec<_>> = source_rings
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|id| {
                        let q = self.vertices[id].position - points[0];
                        roll * (q - first * q.dot(first))
                    })
                    .collect()
            })
            .collect();
        let mut frames = vec![DQuat::IDENTITY];
        for i in 1..directions.len() {
            if directions[i - 1].dot(directions[i]) < -0.9999 {
                return Err(invalid_sweep());
            }
            frames.push(DQuat::from_rotation_arc(directions[i - 1], directions[i]) * frames[i - 1]);
        }
        let mut seam_roll = 0.;
        if closed {
            let last = *directions.last().unwrap();
            if last.dot(first) < -0.9999 {
                return Err(invalid_sweep());
            }
            let transported = DQuat::from_rotation_arc(last, first) * *frames.last().unwrap();
            let u = (plane.u - first * plane.u.dot(first)).normalize();
            let v = transported * u;
            seam_roll = first.dot(v.cross(u)).atan2(v.dot(u));
        }
        let total: f64 = (0..segments)
            .map(|i| points[i].distance(points[(i + 1) % points.len()]))
            .sum();
        let mut distance = 0.;
        let mut sections = Vec::new();
        for i in 0..points.len() {
            let index = i.min(directions.len() - 1);
            let tangent = directions[index];
            let incoming = if i > 0 {
                directions[i - 1]
            } else if closed {
                *directions.last().unwrap()
            } else {
                tangent
            };
            let miter = (incoming + tangent)
                .try_normalize()
                .ok_or_else(invalid_sweep)?;
            let denominator = tangent.dot(miter);
            if denominator < 0.01 {
                return Err(invalid_sweep());
            }
            let frame =
                DQuat::from_axis_angle(tangent, seam_roll * distance / total) * frames[index];
            let section: Vec<Vec<_>> = offsets
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|q| {
                            let q = frame * *q;
                            points[i] + q - tangent * (q.dot(miter) / denominator)
                        })
                        .collect()
                })
                .collect();
            sections.push(section);
            if i < segments {
                distance += points[i].distance(points[(i + 1) % points.len()]);
            }
        }
        // A tight bend must not fold its inner wall back through itself.
        for i in 0..segments {
            for (a, b) in sections[i]
                .iter()
                .flatten()
                .zip(sections[(i + 1) % points.len()].iter().flatten())
            {
                if !a.is_finite() || !b.is_finite() || (*b - *a).dot(directions[i]) <= EPS {
                    return Err(invalid_sweep());
                }
            }
        }
        let shared: BTreeSet<_> = self
            .faces
            .values()
            .filter(|other| other.id != profile)
            .flat_map(|other| std::iter::once(&other.vertices).chain(&other.holes))
            .flat_map(|r| {
                (0..r.len()).map(move |i| {
                    (
                        r[i].min(r[(i + 1) % r.len()]),
                        r[i].max(r[(i + 1) % r.len()]),
                    )
                })
            })
            .filter(|pair| boundary.contains(pair))
            .collect();
        let reuse_start = !closed
            && sections[0].iter().zip(&source_rings).all(|(ring, ids)| {
                ring.iter()
                    .zip(ids)
                    .all(|(p, id)| p.distance(self.vertices[id].position) < EPS)
            });
        if !shared.is_empty() && (!reuse_start || shared.len() != boundary.len()) {
            return Err(tr!("follow_me.attached_profile"));
        }
        let color = self.face_colors.get(&profile).copied();
        let tag = self.entity_tags.get(&profile).copied();
        let mut next = self.clone();
        next.erase(&BTreeSet::from([profile]));
        let mut rings = Vec::new();
        for (i, section) in sections.iter().enumerate() {
            rings.push(if i == 0 && reuse_start {
                source_rings.clone()
            } else {
                section
                    .iter()
                    .map(|ring| ring.iter().map(|p| next.raw_vertex(*p)).collect::<Vec<_>>())
                    .collect::<Vec<_>>()
            });
        }
        let mut result = BTreeSet::new();
        if !closed && shared.is_empty() {
            let mut start = rings[0].clone();
            for ring in &mut start {
                ring.reverse();
            }
            result.insert(next.raw_face(start[0].clone(), start[1..].to_vec(), color));
        }
        if !closed {
            let end = rings.last().unwrap();
            result.insert(next.raw_face(end[0].clone(), end[1..].to_vec(), color));
        }
        for i in 0..segments {
            let end = (i + 1) % points.len();
            for (r, ring) in rings[i].iter().enumerate() {
                for j in 0..ring.len() {
                    let k = (j + 1) % ring.len();
                    let wall = vec![ring[j], ring[k], rings[end][r][k], rings[end][r][j]];
                    if faces::plane(&next, &wall).is_some() {
                        result.insert(next.raw_face(wall, vec![], color));
                    } else {
                        // Spatial paths with closure correction can twist a wall.
                        // Triangles keep every face planar and share the diagonal.
                        result.insert(next.raw_face(
                            vec![wall[0], wall[1], wall[2]],
                            vec![],
                            color,
                        ));
                        result.insert(next.raw_face(
                            vec![wall[0], wall[2], wall[3]],
                            vec![],
                            color,
                        ));
                    }
                }
            }
        }
        // Remove only obsolete profile wires. Preserve the input path and every
        // boundary still supporting another surface or an annotation.
        let used: BTreeSet<_> = next
            .faces
            .values()
            .flat_map(|f| std::iter::once(&f.vertices).chain(&f.holes))
            .flat_map(|r| {
                (0..r.len()).map(move |i| {
                    (
                        r[i].min(r[(i + 1) % r.len()]),
                        r[i].max(r[(i + 1) % r.len()]),
                    )
                })
            })
            .collect();
        let obsolete = next
            .edges
            .values()
            .filter(|e| {
                let pair = (e.a.min(e.b), e.a.max(e.b));
                boundary.contains(&pair) && !used.contains(&pair) && !path.contains(&e.id)
            })
            .map(|e| e.id)
            .collect();
        next.erase(&obsolete);
        if let Some(tag) = tag {
            for id in &result {
                next.entity_tags.insert(*id, tag);
            }
        }
        validate_surface(&next, &result, shared.is_empty())?;
        next.validate()?;
        *self = next;
        Ok(result)
    }
}

/// A bounded broad phase rejects intersections between distant portions of the
/// sweep. Local folds are rejected earlier by the segment-direction test.
fn validate_surface(
    model: &Model,
    result: &BTreeSet<u64>,
    closed_shell: bool,
) -> Result<(), &'static str> {
    let mut uses = BTreeMap::new();
    let mut triangles = Vec::new();
    for id in result {
        let face = &model.faces[id];
        for ring in std::iter::once(&face.vertices).chain(&face.holes) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        for t in model.face_triangles(face) {
            triangles.push((t, t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2])));
        }
    }
    if closed_shell && uses.values().any(|n| *n != 2) {
        return Err(invalid_sweep());
    }
    triangles.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
    let mut active: Vec<usize> = Vec::new();
    let mut comparisons = 0;
    for i in 0..triangles.len() {
        let (a, lo, hi) = triangles[i];
        active.retain(|j| triangles[*j].2.x >= lo.x - EPS);
        for j in &active {
            comparisons += 1;
            if comparisons > 2_000_000 {
                return Err(tr!("follow_me.too_large"));
            }
            let (b, b_lo, b_hi) = triangles[*j];
            if lo.y > b_hi.y + EPS
                || lo.z > b_hi.z + EPS
                || b_lo.y > hi.y + EPS
                || b_lo.z > hi.z + EPS
            {
                continue;
            }
            // Neighbor triangles are allowed to meet at their shared topology.
            if a.iter().any(|p| b.iter().any(|q| p.distance(*q) < EPS)) {
                continue;
            }
            if triangles_intersect(a, b) {
                return Err(invalid_sweep());
            }
        }
        active.push(i);
    }
    Ok(())
}

fn triangles_intersect(a: [DVec3; 3], b: [DVec3; 3]) -> bool {
    let n = (b[1] - b[0]).cross(b[2] - b[0]).normalize();
    if a.iter().all(|p| (*p - b[0]).dot(n).abs() < EPS) {
        let u = (b[1] - b[0]).normalize();
        let v = n.cross(u);
        let project = |p: DVec3| crate::DVec2::new((p - b[0]).dot(u), (p - b[0]).dot(v));
        let a = a.map(project);
        let b = b.map(project);
        return faces::in_polygon(a[0], &b, true)
            || faces::in_polygon(b[0], &a, true)
            || (0..3).any(|i| {
                (0..3).any(|j| {
                    crate::selection::segment_distance(a[i], a[(i + 1) % 3], b[j], b[(j + 1) % 3])
                        < EPS
                })
            });
    }
    fn crosses(a: [DVec3; 3], b: [DVec3; 3]) -> bool {
        let u = b[1] - b[0];
        let v = b[2] - b[0];
        let n = u.cross(v);
        (0..3).any(|i| {
            let p = a[i];
            let edge = a[(i + 1) % 3] - p;
            let denominator = edge.dot(n);
            if denominator.abs() < 1e-14 {
                return false;
            }
            let t = (b[0] - p).dot(n) / denominator;
            if !(0.0..=1.0).contains(&t) {
                return false;
            }
            let q = p + edge * t - b[0];
            let x = q.cross(v).dot(n) / n.length_squared();
            let y = u.cross(q).dot(n) / n.length_squared();
            x >= -1e-9 && y >= -1e-9 && x + y <= 1. + 1e-9
        })
    }
    crosses(a, b) || crosses(b, a)
}

fn ordered_path(
    model: &Model,
    edges: &BTreeSet<u64>,
    center: DVec3,
    normal: DVec3,
    reverse: bool,
) -> Result<(Vec<DVec3>, bool), &'static str> {
    let mut neighbors: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for id in edges {
        let e = &model.edges[id];
        if model.edge_length(e) < EPS {
            return Err(invalid_path());
        }
        neighbors.entry(e.a).or_default().push(e.b);
        neighbors.entry(e.b).or_default().push(e.a);
    }
    if neighbors.values().any(|n| n.len() > 2) {
        return Err(invalid_path());
    }
    let ends: Vec<_> = neighbors
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(id, _)| *id)
        .collect();
    let closed = ends.is_empty();
    if !closed && ends.len() != 2 {
        return Err(invalid_path());
    }
    let candidates: Vec<_> = if closed {
        neighbors.keys().copied().collect()
    } else {
        ends
    };
    let start = *candidates
        .iter()
        .min_by(|a, b| {
            model.vertices[*a]
                .position
                .distance_squared(center)
                .total_cmp(&model.vertices[*b].position.distance_squared(center))
        })
        .ok_or_else(invalid_path)?;
    let mut ids = vec![start];
    let mut visited = BTreeSet::from([start]);
    let mut previous = None;
    let mut current = start;
    loop {
        let next = if previous.is_none() && closed {
            neighbors[&current].iter().copied().max_by(|a, b| {
                let direction = |id: &u64| {
                    (model.vertices[id].position - model.vertices[&start].position)
                        .normalize()
                        .dot(normal)
                };
                direction(a)
                    .abs()
                    .total_cmp(&direction(b).abs())
                    .then_with(|| direction(a).total_cmp(&direction(b)))
            })
        } else {
            neighbors[&current]
                .iter()
                .copied()
                .find(|id| Some(*id) != previous)
        };
        let Some(next) = next else {
            break;
        };
        if next == start {
            break;
        }
        if !visited.insert(next) {
            return Err(invalid_path());
        }
        ids.push(next);
        previous = Some(current);
        current = next;
    }
    if ids.len() != neighbors.len() || edges.len() != ids.len() - usize::from(!closed) {
        return Err(invalid_path());
    }
    if reverse {
        if closed {
            ids[1..].reverse();
        } else {
            ids.reverse();
        }
    }
    Ok((
        ids.iter().map(|id| model.vertices[id].position).collect(),
        closed,
    ))
}

#[cfg(test)]
#[path = "follow_me_tests.rs"]
mod tests;
