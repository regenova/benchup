//! Input and rendering for the modeling viewport.
use super::*;
use benchup_core::tr;
use std::collections::BTreeMap;

#[path = "quick_views.rs"]
mod quick_views;
pub(super) use quick_views::ViewTransition;

/// Tool options share Orbit's fixed top-right overlay in the viewport.
fn viewport_tool_controls(
    ctx: &egui::Context,
    rect: Rect,
    id: egui::Id,
    contents: impl FnOnce(&mut egui::Ui, f32),
) {
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(rect.right_top() + egui::vec2(-8., 8.))
        .movable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let control_height = ui.spacing().interact_size.y.max(
                        ui.text_style_height(&egui::TextStyle::Button).max(18.)
                            + 2. * ui.spacing().button_padding.y,
                    );
                    ui.spacing_mut().interact_size.y = control_height;
                    let colors = Palette::for_ui(ui);
                    let widgets = &mut ui.style_mut().visuals.widgets;
                    for (visuals, opacity) in [
                        (&mut widgets.inactive, 0.75),
                        (&mut widgets.hovered, 0.80),
                        (&mut widgets.active, 0.80),
                    ] {
                        let fill = colors.paper.gamma_multiply(opacity);
                        visuals.bg_fill = fill;
                        visuals.weak_bg_fill = fill;
                    }
                    contents(ui, control_height);
                });
            });
        });
}

#[derive(Default)]
pub(super) struct TouchZoom {
    contacts: BTreeMap<(egui::TouchDeviceId, egui::TouchId), (Pos2, bool)>,
    started_in_viewport: bool,
    captured: bool,
    baseline: Option<(f32, Pos2)>,
}

impl TouchZoom {
    fn pair(&self) -> Option<(f32, Pos2)> {
        if self.contacts.len() != 2 {
            return None;
        }
        let mut contacts = self.contacts.iter();
        let (a_id, a) = contacts.next()?;
        let (b_id, b) = contacts.next()?;
        (a_id.0 == b_id.0 && a.1 && b.1 && self.started_in_viewport)
            .then_some((a.0.distance(b.0), a.0 + (b.0 - a.0) * 0.5))
    }

    fn motion(&mut self) -> (Vec2, f64, Option<Pos2>) {
        let Some((distance, center)) = self.pair() else {
            self.baseline = None;
            return (Vec2::ZERO, 0., None);
        };
        let (old_distance, old_center) = *self.baseline.get_or_insert((distance, center));
        let translation = center - old_center;
        let separation = (distance - old_distance).abs();
        // Compare complete contact snapshots, never individual finger events:
        // otherwise parallel movement briefly looks like a pinch. Accumulate
        // sub-threshold movement so slow deliberate gestures still work.
        let zooming = old_distance > 1.
            && distance > 1.
            && separation
                > 2.0_f32
                    .max(old_distance * 0.01)
                    .max(translation.length() * 0.3);
        let panning = translation.length() > 2.0_f32.max(separation * 0.35);
        if zooming || panning {
            self.baseline = Some((distance, center));
        }
        (
            if panning { translation } else { Vec2::ZERO },
            if zooming {
                (distance as f64 / old_distance as f64).ln() / 0.002
            } else {
                0.
            },
            Some(center),
        )
    }
}

impl BenchUpApp {
    // Read raw contacts before synthesized pointer events reach modeling tools.
    // Capture lasts until every finger lifts, including cancellation and 2 -> 1.
    fn touch_viewport_zoom(&mut self, ctx: &egui::Context, response: &egui::Response) -> bool {
        if self.tool == Tool::Walk {
            return false;
        }
        let allowed = response.enabled()
            && ctx.input(|i| i.focused)
            && !self.project.blocked()
            && !self.screenshot.open
            && !self.settings_open
            && !self.help
            && !self.about
            && !self.release_notes.open
            && self.object_creation.is_none()
            && self.object_rename.is_none()
            && self.text_editor.is_none();
        #[cfg(not(target_arch = "wasm32"))]
        let allowed = allowed && !self.updates.open;
        let state = &mut self.touch_zoom;
        let mut suppress = state.captured;

        for event in ctx.input(|i| i.events.clone()) {
            let egui::Event::Touch {
                device_id,
                id,
                phase,
                pos,
                ..
            } = event
            else {
                continue;
            };
            let key = (device_id, id);
            let old_count = state.contacts.len();
            match phase {
                egui::TouchPhase::Start => {
                    let in_viewport = allowed
                        && response.rect.contains(pos)
                        && ctx.layer_id_at(pos) == Some(response.layer_id);
                    state.contacts.insert(key, (pos, in_viewport));
                    if state.contacts.len() >= 2 {
                        state.captured |=
                            state.contacts.values().any(|(_, in_viewport)| *in_viewport);
                        if state.contacts.len() == 2 {
                            state.started_in_viewport |=
                                state.contacts.values().all(|(_, inside)| *inside);
                        }
                    }
                }
                egui::TouchPhase::Move => {
                    if let Some(contact) = state.contacts.get_mut(&key) {
                        contact.0 = pos;
                    }
                }
                egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                    state.contacts.remove(&key);
                }
            }
            if state.contacts.len() != old_count {
                // Rebase on membership changes, including 3 -> 2.
                state.baseline = state.pair();
            }
            suppress |= state.captured;
        }
        if !allowed {
            state.started_in_viewport = false;
            state.baseline = None;
            if !ctx.input(|i| i.focused) {
                state.contacts.clear();
            }
        }
        let (pan, scroll, anchor) = if allowed {
            state.motion()
        } else {
            (Vec2::ZERO, 0., None)
        };
        if state.contacts.is_empty() {
            state.started_in_viewport = false;
            state.captured = false;
        }
        if suppress {
            self.gesture = None;
            self.section_drag = None;
            if self.tool == Tool::Freehand {
                self.drawing = None;
            }
            self.viewport_pointer = None;
            self.hover = None;
            self.flip_hover = None;
            self.drawing_orbit_pivot = None;
            if pan != Vec2::ZERO {
                self.interrupt_view_transition(ctx);
                self.doc.camera.pan(
                    dv(pan) * self.general_settings.mouse_pan_speed(),
                    self.viewport.height() as f64,
                );
                self.camera_changed(ctx);
            }
            if scroll.abs() > 1e-9
                && let Some(anchor) = anchor
            {
                self.interrupt_view_transition(ctx);
                self.zoom_camera_at(scroll, anchor);
                self.camera_changed(ctx);
            }
        }
        suppress
    }

    pub(super) fn viewport_workspace(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|input| input.time);
        let rect = ui.available_rect_before_wrap();
        self.viewport = rect;
        let response = ui.interact(
            rect,
            ui.scope_id().with("viewport-canvas"),
            Sense::click_and_drag(),
        );
        #[cfg(target_arch = "wasm32")]
        if let Some(canvas) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("benchup-canvas"))
        {
            let scale = ctx.pixels_per_point()
                / web_sys::window()
                    .map(|w| w.device_pixel_ratio() as f32)
                    .unwrap_or(1.);
            let bounds =
                (response.enabled() && rect.width() > 1. && rect.height() > 1.).then_some([
                    rect.min.x * scale,
                    rect.min.y * scale,
                    rect.max.x * scale,
                    rect.max.y * scale,
                ]);
            let _ = canvas.set_attribute(
                "data-model-viewport",
                &serde_json::to_string(&bounds).unwrap_or_default(),
            );
        }
        if rect.width() <= 1. || rect.height() <= 1. {
            return;
        }
        let view_bar = self.quick_view_bar(ui, rect);
        self.viewport_navigation_options(&ctx, rect, view_bar);
        let touch_navigation = self.touch_viewport_zoom(&ctx, &response);
        if self.view_transition.is_some()
            && response.enabled()
            && response.contains_pointer()
            && ctx.input(|input| {
                input.pointer.any_pressed()
                    || input.smooth_scroll_delta.y != 0.
                    || input.key_down(Key::V) && self.general_settings.hold_v_to_orbit
                    || self.drawing_orbit_down
            })
        {
            self.interrupt_view_transition(&ctx);
        }
        let motion = self
            .view_transition
            .as_ref()
            .and_then(|transition| transition.sample(&self.doc.camera, now));
        let transitioning = motion.is_some();
        if transitioning {
            ctx.request_repaint();
        } else {
            self.view_transition = None;
            if !touch_navigation {
                self.viewport_input(&ctx, &response);
            }
        }
        if !touch_navigation
            && !ctx
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|pos| self.compass_contains(pos))
            && self.tool != Tool::Walk
        {
            self.context_menu(&response);
        }

        // Only the rendered frame uses the interpolated camera. The document
        // and the shared preferences keep the exact destination.
        let destination = motion.map(|camera| std::mem::replace(&mut self.doc.camera, camera));
        if self.gpu_ready {
            self.paint_scene(ui, rect);
        } else {
            ui.painter().with_clip_rect(rect).text(
                rect.center(),
                Align2::CENTER_CENTER,
                tr!("viewports.a_compatible_gpu_renderer_is_required_try_updating"),
                FontId::proportional(15.),
                ACCENT,
            );
        }
        if !touch_navigation && !transitioning {
            self.fastener_face_handles(ui, rect);
            self.joint_marker_handles(ui, rect);
        }
        self.paint_shared_presence(ui, rect);
        self.paint_compass(ui, &response);
        self.paint_section_handles(ui, &response);
        crate::environment::paint_sun_hint(ui.painter(), rect, &self.doc.camera, &self.doc.display);
        if self.general_settings.show_fps {
            crate::frame_rate::paint(ui, rect);
        } else {
            crate::frame_rate::reset(&ctx);
        }
        self.paint_tool_cursor(&ctx, &response);
        if let Some(camera) = destination {
            self.doc.camera = camera;
        }
    }

    fn viewport_navigation_options(&mut self, ctx: &egui::Context, rect: Rect, view_bar: Rect) {
        let id = match self.tool {
            Tool::Orbit => "orbit-origin-selector",
            Tool::Polygon => "polygon-options",
            Tool::FollowMe => "follow-me-options",
            _ => return,
        };
        let mut tool_rect = rect;
        if rect.right() - view_bar.right() < 245. {
            tool_rect.min.y = view_bar.bottom();
        }
        viewport_tool_controls(
            ctx,
            tool_rect,
            egui::Id::unique(id),
            |ui, control_height| {
                if self.tool == Tool::FollowMe {
                    self.follow_me_options(ui, control_height);
                    return;
                }
                if self.tool == Tool::Polygon {
                    ui.add(
                        egui::DragValue::new(&mut self.polygon_sides)
                            .range(3..=64)
                            .min_size(egui::vec2(0., control_height))
                            .prefix(format!("{}: ", tr!("app.polygon_sides"))),
                    )
                    .on_hover_text(tr!("app.polygon_sides"));
                    return;
                }
                if ui
                    .add(
                        egui::Button::new(tr!("orbit.set_focus"))
                            .selected(self.setting_orbit_focus)
                            .min_size(egui::vec2(0., control_height)),
                    )
                    .on_hover_text(tr!("orbit.set_focus_hint"))
                    .clicked()
                {
                    self.setting_orbit_focus = !self.setting_orbit_focus;
                }
                let colors = Palette::for_ui(ui);
                let icon = crate::icons::origin_atom(colors.ink);
                egui::ComboBox::from_id_salt("orbit-origin")
                    .selected_text((icon, self.orbit_origin.label()))
                    .show_ui(ui, |ui| {
                        for origin in sidebar::OrbitOrigin::ALL {
                            ui.selectable_value(&mut self.orbit_origin, origin, origin.label());
                        }
                    })
                    .response
                    .on_hover_text(tr!("orbit.pivot_hint"));
            },
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "viewport_tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "touch_navigation_tests.rs"]
mod touch_navigation_tests;
