use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Preferences {
    pub roll: f64,
    pub reverse: bool,
}

#[derive(Default)]
pub(super) struct State {
    pub preferences: Preferences,
    profile: Option<u64>,
    edges: Vec<u64>,
    adjacency: Option<BTreeMap<u64, Vec<u64>>>,
    pub preview: Option<Model>,
    result: BTreeSet<u64>,
    error: Option<&'static str>,
    #[cfg(test)]
    preview_updates: usize,
}

impl State {
    pub fn is_tracing(&self) -> bool {
        self.profile.is_some()
    }

    pub fn reset(&mut self) {
        let preferences = self.preferences;
        *self = Self {
            preferences,
            ..Default::default()
        };
    }
}

impl BenchUpApp {
    pub(super) fn follow_me_options(&mut self, ui: &mut egui::Ui, control_height: f32) {
        let previous = self.follow_me.preferences;
        ui.add(
            egui::DragValue::new(&mut self.follow_me.preferences.roll)
                .speed(1.)
                .range(-180.0..=180.0)
                .min_size(egui::vec2(0., control_height))
                .prefix(format!("{}: ", tr!("follow_me.roll")))
                .suffix("°"),
        )
        .on_hover_text(tr!("follow_me.roll"));
        if ui
            .add(
                egui::Button::new(tr!("follow_me.reverse"))
                    .selected(self.follow_me.preferences.reverse)
                    .min_size(egui::vec2(0., control_height)),
            )
            .clicked()
        {
            self.follow_me.preferences.reverse = !self.follow_me.preferences.reverse;
        }
        if previous != self.follow_me.preferences {
            self.refresh_follow_me_preview();
        }
    }

    fn follow_me_selection(&self, profile: u64) -> BTreeSet<u64> {
        self.selected
            .iter()
            .copied()
            .filter(|id| *id != profile)
            .collect()
    }

    fn follow_me_face_at(&mut self, pos: Pos2) -> Option<u64> {
        self.prepare_picking();
        let ray = self
            .doc
            .camera
            .ray(dv(pos - self.viewport.min), dv(self.viewport.size()));
        let (id, _, _) =
            self.picking_cache
                .nearest(&self.face_cache, &self.entity_cache, ray, false)?;
        let state = self.entity_cache.get(id);
        (state.context == Some(id)
            && !state.hidden
            && !self.doc.model.reference_images.contains_key(&id))
        .then_some(id)
    }

    fn follow_me_edge_at(&mut self, pos: Pos2) -> Option<u64> {
        self.prepare_picking();
        let (id, _) = self.picking_cache.pick_edge(
            &self.doc.model,
            &self.face_cache,
            &self.entity_cache,
            &self.doc.camera,
            dv(self.viewport.size()),
            dv(pos - self.viewport.min),
            8.,
        )?;
        let state = self.entity_cache.get(id);
        (state.context == Some(id) && !state.hidden && !state.locked).then_some(id)
    }

    pub(super) fn follow_me_pointer(&mut self, pos: Pos2) {
        self.snap = None;
        if self.follow_me.profile.is_none() {
            self.hover = self.follow_me_face_at(pos);
            return;
        }
        self.hover = self.follow_me_edge_at(pos);
        if let Some(edge) = self.hover {
            self.trace_follow_me_edge(edge);
        }
    }

    fn trace_follow_me_edge(&mut self, edge: u64) {
        let Some(profile) = self
            .follow_me
            .profile
            .and_then(|id| self.doc.model.faces.get(&id))
        else {
            return;
        };
        let model = &self.doc.model;
        let e = &model.edges[&edge];
        // A profile boundary cannot also be the extrusion path.
        if std::iter::once(&profile.vertices)
            .chain(&profile.holes)
            .any(|ring| {
                (0..ring.len()).any(|i| {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    (e.a == a && e.b == b) || (e.a == b && e.b == a)
                })
            })
        {
            return;
        }
        if self.follow_me.edges.last() == Some(&edge) {
            return;
        }
        if self
            .follow_me
            .edges
            .iter()
            .any(|id| !model.edges.contains_key(id))
        {
            self.follow_me.reset();
            return;
        }
        if let Some(index) = self.follow_me.edges.iter().position(|id| *id == edge) {
            if index == 0 && self.follow_me.edges.len() > 2 {
                let mut degree = BTreeMap::new();
                for id in &self.follow_me.edges {
                    let e = &model.edges[id];
                    *degree.entry(e.a).or_insert(0) += 1;
                    *degree.entry(e.b).or_insert(0) += 1;
                }
                if degree.values().all(|n| *n == 2) {
                    return;
                }
            }
            // Moving back along the trace shortens the preview.
            self.follow_me.edges.truncate(index + 1);
        } else {
            if let Some(last) = self.follow_me.edges.last() {
                let last = &model.edges[last];
                if ![last.a, last.b].iter().any(|v| *v == e.a || *v == e.b) {
                    // Pointer events can skip tiny arc segments. Walk only
                    // unbranched continuations, stopping at a junction instead
                    // of guessing which branch the user meant.
                    if self.follow_me.adjacency.is_none() {
                        let mut adjacency: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
                        for candidate in model.edges.values() {
                            let state = self.entity_cache.get(candidate.id);
                            if state.context == Some(candidate.id) && !state.hidden && !state.locked
                            {
                                adjacency.entry(candidate.a).or_default().push(candidate.id);
                                adjacency.entry(candidate.b).or_default().push(candidate.id);
                            }
                        }
                        self.follow_me.adjacency = Some(adjacency);
                    }
                    let adjacency = self.follow_me.adjacency.as_ref().unwrap();
                    let previous = self
                        .follow_me
                        .edges
                        .iter()
                        .rev()
                        .nth(1)
                        .map(|id| &model.edges[id]);
                    let extension = [last.a, last.b]
                        .into_iter()
                        .filter(|vertex| previous.is_none_or(|e| e.a != *vertex && e.b != *vertex))
                        .find_map(|mut vertex| {
                            let mut previous = last.id;
                            let mut extension = Vec::new();
                            let mut visited: BTreeSet<_> =
                                self.follow_me.edges.iter().copied().collect();
                            while visited.len() < 4096 {
                                let candidates: Vec<_> = adjacency
                                    .get(&vertex)?
                                    .iter()
                                    .copied()
                                    .filter(|id| *id != previous)
                                    .collect();
                                if candidates.len() != 1 {
                                    return None;
                                }
                                let candidate = candidates[0];
                                if !visited.insert(candidate) {
                                    return None;
                                }
                                extension.push(candidate);
                                if candidate == edge {
                                    return Some(extension);
                                }
                                let e = model.edges.get(&candidate)?;
                                vertex = if e.a == vertex { e.b } else { e.a };
                                previous = candidate;
                            }
                            None
                        });
                    let Some(extension) = extension else {
                        return;
                    };
                    self.follow_me.edges.extend(extension);
                    self.refresh_follow_me_preview();
                    return;
                }
            }
            self.follow_me.edges.push(edge);
        }
        self.refresh_follow_me_preview();
    }

    fn refresh_follow_me_preview(&mut self) {
        #[cfg(test)]
        {
            self.follow_me.preview_updates += 1;
        }
        self.follow_me.preview = None;
        self.follow_me.result.clear();
        self.follow_me.error = None;
        let Some(profile) = self.follow_me.profile else {
            return;
        };
        if self.follow_me.edges.is_empty() {
            return;
        }
        let mut preview = self.doc.model.clone();
        let settings = self.follow_me.preferences;
        match preview.follow_me(
            profile,
            &self.follow_me.edges.iter().copied().collect(),
            settings.roll,
            settings.reverse,
        ) {
            Ok(result) => {
                self.follow_me.preview = Some(preview);
                self.follow_me.result = result;
            }
            Err(message) => self.follow_me.error = Some(message),
        }
    }

    pub(super) fn follow_me_click(&mut self, pos: Pos2, ctx: &egui::Context) {
        if self.follow_me.profile.is_some() {
            self.follow_me_pointer(pos);
            // Revalidate against the current document at commit, including any
            // collaboration changes received while the preview was displayed.
            self.refresh_follow_me_preview();
            if let Some(preview) = self.follow_me.preview.take() {
                let result = std::mem::take(&mut self.follow_me.result);
                self.commit_follow_me(preview, result, ctx);
            } else {
                self.say(ctx, self.follow_me.error.unwrap_or(tr!("follow_me.trace")));
            }
            return;
        }
        let Some(profile) = self.follow_me_face_at(pos) else {
            self.say(ctx, tr!("follow_me.pick_profile"));
            return;
        };
        let path = self.follow_me_selection(profile);
        if path.is_empty() {
            if self.doc.model.is_locked(profile) {
                self.say(
                    ctx,
                    tr!("core.modeling.unlock_the_object_before_editing_it"),
                );
            } else {
                self.follow_me.profile = Some(profile);
                self.say(ctx, tr!("follow_me.trace"));
            }
            return;
        }
        let settings = self.follow_me.preferences;
        let mut model = self.doc.model.clone();
        match model.follow_me(profile, &path, settings.roll, settings.reverse) {
            Ok(result) => self.commit_follow_me(model, result, ctx),
            Err(message) => self.say(ctx, message),
        }
    }

    fn commit_follow_me(&mut self, model: Model, result: BTreeSet<u64>, ctx: &egui::Context) {
        if !self.shared_edit_allowed(ctx) {
            self.say(ctx, self.shared_edit_block_reason(ctx));
            return;
        }
        let before = std::mem::replace(&mut self.doc.model, model);
        self.follow_me.reset();
        self.selected = result;
        self.hover = None;
        self.notice = None;
        self.finish_edit(tr!("follow_me.tool"), before, ctx);
    }

    pub(super) fn follow_me_status(&self) -> &'static str {
        self.follow_me.error.unwrap_or_else(|| {
            if self.follow_me.profile.is_some() {
                tr!("follow_me.trace")
            } else {
                Tool::FollowMe.instructions()
            }
        })
    }
}

#[cfg(test)]
#[path = "follow_me_tool_tests.rs"]
mod tests;
