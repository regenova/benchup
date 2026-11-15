use super::*;

/// A user-defined machine envelope — a 3D printer build volume, a CNC working
/// area, or a planer's capacity — drawn as a locked wireframe reference in the
/// viewport.
///
/// The volume is a global preference, not document geometry: users print on the
/// same printer across projects. Because it is never part of `Model`, it cannot
/// be selected, edited, or moved, which satisfies the "locked" requirement
/// without any locking machinery.
/// Limits in metres. One millimetre up to a hundred metres covers a printer bed,
/// a CNC envelope and a workshop-sized tool.
pub(super) const MIN_M: f64 = 0.001;
pub(super) const MAX_M: f64 = 100.;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    /// Show the toggle on the viewport toolbar.
    pub show_button: bool,
    /// Draw the wireframe box in the viewport.
    pub visible: bool,
    /// Width (X), depth (Y) and height (Z) in metres, matching the model space.
    /// The settings UI converts to and from the user's display units, so a user
    /// working in feet sees and enters feet while the stored value stays exact.
    pub size_m: [f64; 3],
    /// Optional shortcut that toggles `visible`. Unassigned by default.
    pub hotkey: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_button: true,
            visible: false,
            // 256 mm cubed: large enough to read clearly at the default viewport
            // zoom. Only a starting point; the user sets their own machine.
            size_m: [0.256, 0.256, 0.256],
            hotkey: None,
        }
    }
}

impl Settings {
    /// Keep saved preferences usable. Older files lack these fields entirely and
    /// `#[serde(default)]` supplies them; damaged values fall back per axis so one
    /// bad dimension never discards the others.
    pub(super) fn sanitize(&mut self) {
        let fallback = Self::default().size_m;
        for (value, default) in self.size_m.iter_mut().zip(fallback) {
            if !value.is_finite() {
                *value = default;
            }
            *value = value.clamp(MIN_M, MAX_M);
        }
        if let Some(key) = &self.hotkey
            && !super::shortcut_input::valid_tool_key(key)
        {
            self.hotkey = None;
        }
        self.enforce();
    }

    /// Hiding the viewport toggle always hides the box. Keeping the box alive
    /// because a shortcut happens to exist surprised users: turning the setting
    /// off left the volume on screen. A configured shortcut can still switch the
    /// box back on afterwards.
    pub(super) fn enforce(&mut self) {
        if !self.show_button {
            self.visible = false;
        }
    }

    pub(super) fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// A stored axis length expressed in `unit`. The value is always held in
    /// metres, so switching display units re-presents the same physical volume.
    pub(super) fn axis_in(&self, axis: usize, unit: benchup_core::LengthUnit) -> f64 {
        self.size_m[axis] / unit.factor()
    }

    /// Store an axis length entered in `unit`.
    pub(super) fn set_axis_from(
        &mut self,
        axis: usize,
        value: f64,
        unit: benchup_core::LengthUnit,
    ) {
        self.size_m[axis] = value * unit.factor();
    }

    /// Half extent of the volume in metres.
    fn half_extent(&self) -> DVec3 {
        DVec3::new(self.size_m[0], self.size_m[1], self.size_m[2]) * 0.5
    }

    /// The occupied region in model space: centred on the origin in X and Y, and
    /// sitting on Z = 0. That puts the bottom face's centre at the origin.
    pub(super) fn bounds(&self) -> (DVec3, DVec3) {
        let half = self.half_extent();
        (
            DVec3::new(-half.x, -half.y, 0.),
            DVec3::new(half.x, half.y, self.size_m[2]),
        )
    }

    /// True when the given bounds fit inside the volume. Touching counts as
    /// fitting, so a model printed exactly to the bed size is not flagged.
    pub(super) fn contains(&self, low: DVec3, high: DVec3) -> bool {
        let (min, max) = self.bounds();
        let tolerance = 1e-9;
        (0..3).all(|axis| {
            low[axis] >= min[axis] - tolerance && high[axis] <= max[axis] + tolerance
        })
    }

    /// Any geometry outside the volume means a vertex is outside, and therefore
    /// that the model's bounds leave the volume, so one bounds test is exact and
    /// far cheaper than walking every vertex.
    pub(super) fn exceeds(&self, model: &Model) -> bool {
        model
            .bounds()
            .is_some_and(|(low, high)| !self.contains(low, high))
    }

    pub(super) fn hotkey(&self) -> Option<egui::Key> {
        self.hotkey
            .as_deref()
            .and_then(egui::Key::from_name)
    }

    /// Draw the 12 edges through the retained overlay path, so the box costs no
    /// CPU geometry per frame and shares the depth behaviour of other overlays.
    pub(super) fn paint(&self, mesh: &mut renderer::Mesh) {
        let (min, max) = self.bounds();
        let corners = [
            DVec3::new(min.x, min.y, min.z),
            DVec3::new(max.x, min.y, min.z),
            DVec3::new(max.x, max.y, min.z),
            DVec3::new(min.x, max.y, min.z),
            DVec3::new(min.x, min.y, max.z),
            DVec3::new(max.x, min.y, max.z),
            DVec3::new(max.x, max.y, max.z),
            DVec3::new(min.x, max.y, max.z),
        ];
        const EDGES: [(usize, usize); 12] = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        for (a, b) in EDGES {
            mesh.overlay_line(corners[a], corners[b], 1.5, ACCENT, true);
        }
    }
}

impl BenchUpApp {
    /// Viewport toolbar toggle. Hidden entirely when the user opts out, which is
    /// the issue's "show/hide the button on the canvas bar" setting.
    pub(super) fn printer_volume_button(&mut self, ui: &mut egui::Ui) {
        if !self.general_settings.volume.show_button {
            return;
        }
        let colors = Palette::for_ui(ui);
        let active = self.general_settings.volume.visible;
        let response = ui.add(
            egui::Button::new("")
                .min_size(egui::vec2(32., 32.))
                .selected(active),
        );
        icons::line_icon(
            ui.painter(),
            response.rect.center(),
            "printer_volume",
            if active { colors.ink } else { colors.muted },
        );
        let response =
            response.on_hover_text(tr!("app.show_or_hide_the_printer_or_tool_volume"));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                true,
                tr!("app.show_or_hide_the_printer_or_tool_volume"),
            )
        });
        if response.clicked() {
            response.surrender_focus();
            self.toggle_printer_volume(ui.ctx());
        }
    }

    /// Toggling on frames the volume, because a real machine envelope is only a few
    /// pixels wide at the default zoom. Toggling off leaves the camera untouched.
    pub(super) fn toggle_printer_volume(&mut self, ctx: &egui::Context) {
        self.general_settings.volume.toggle();
        if self.general_settings.volume.visible {
            self.doc.camera.fit(
                Some(self.general_settings.volume.bounds()),
                dv(self.viewport.size().max(Vec2::splat(1.))),
            );
            self.changed(ctx);
        }
        ctx.request_repaint();
    }

    /// `Model::bounds` expands visibility and walks geometry, so it must not run
    /// every frame. Cache the answer against the caller's scene revision, which
    /// advances on every model change, plus the volume's own dimensions.
    pub(super) fn volume_exceeded(&self, ctx: &egui::Context, revision: u64) -> bool {
        if !self.general_settings.volume.visible {
            return false;
        }
        let id = egui::Id::unique("printer_volume_warning");
        let size = self.general_settings.volume.size_m;
        if let Some((cached_revision, cached_size, exceeded)) =
            ctx.data(|data| data.get_temp::<(u64, [f64; 3], bool)>(id))
            && cached_revision == revision
            && cached_size == size
        {
            return exceeded;
        }
        let exceeded = self.general_settings.volume.exceeds(&self.doc.model);
        ctx.data_mut(|data| data.insert_temp(id, (revision, size, exceeded)));
        exceeded
    }

    /// Persistent viewport banner while any geometry leaves the volume. It is
    /// gated on the box being visible, so hiding the ghost also hides the warning.
    pub(super) fn paint_volume_warning(
        &self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        rect: Rect,
        paper: Color32,
        ink: Color32,
        revision: u64,
    ) {
        if !self.volume_exceeded(ctx, revision) {
            return;
        }
        let galley = painter.layout(
            tr!("app.the_model_is_outside_the_printer_or_tool_volume").to_owned(),
            FontId::proportional(12.),
            ink,
            (rect.width() - 80.).clamp(160., 520.),
        );
        let banner = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 30.),
            galley.size() + egui::vec2(24., 16.),
        );
        painter.rect_filled(banner, 7, paper);
        painter.rect_stroke(
            banner,
            7,
            Stroke::new(1., ink.gamma_multiply(0.5)),
            egui::StrokeKind::Inside,
        );
        painter.galley(banner.min + egui::vec2(12., 8.), galley, ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_hidden_with_a_visible_button() {
        let settings = Settings::default();
        assert!(settings.show_button);
        assert!(!settings.visible);
        assert_eq!(settings.hotkey, None);
        assert_eq!(settings.size_m, [0.256, 0.256, 0.256]);
    }

    #[test]
    fn bottom_face_center_sits_on_the_origin() {
        let settings = Settings {
            size_m: [0.2, 0.1, 0.05],
            ..Default::default()
        };
        let (min, max) = settings.bounds();
        assert_eq!(min.x, -0.1);
        assert_eq!(max.x, 0.1);
        assert_eq!(min.y, -0.05);
        assert_eq!(max.y, 0.05);
        // The base rests on the ground plane and the height extends upward.
        assert_eq!(min.z, 0.);
        assert!((max.z - 0.05).abs() < 1e-12);
        assert!((min.x + max.x).abs() < 1e-12, "X is centred on the origin");
        assert!((min.y + max.y).abs() < 1e-12, "Y is centred on the origin");
    }

    #[test]
    fn touching_the_volume_still_fits() {
        let settings = Settings {
            size_m: [0.1, 0.1, 0.1],
            ..Default::default()
        };
        let (min, max) = settings.bounds();
        assert!(settings.contains(min, max));
        assert!(settings.contains(min, max + DVec3::splat(1e-12)));
    }

    #[test]
    fn geometry_outside_the_volume_is_detected() {
        let settings = Settings {
            size_m: [0.1, 0.1, 0.1],
            ..Default::default()
        };
        let (min, max) = settings.bounds();
        // Above the height.
        assert!(!settings.contains(min, max + DVec3::Z * 0.001));
        // Below the bed.
        assert!(!settings.contains(min - DVec3::Z * 0.001, max));
        // Beyond a side.
        assert!(!settings.contains(min, max + DVec3::X * 0.001));
        assert!(!settings.contains(min - DVec3::Y * 0.001, max));
    }

    #[test]
    fn damaged_sizes_fall_back_per_axis_and_extremes_clamp() {
        // One damaged axis keeps the user's other two values.
        let mut settings = Settings {
            size_m: [f64::NAN, 0.01, 0.01],
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.size_m, [Settings::default().size_m[0], 0.01, 0.01]);

        let mut settings = Settings {
            size_m: [-5., 1e9, 0.05],
            hotkey: Some("nope".into()),
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.size_m, [MIN_M, MAX_M, 0.05]);
        assert_eq!(settings.hotkey, None, "an invalid key is dropped");
    }

    #[test]
    fn valid_hotkey_survives_sanitize() {
        let mut settings = Settings {
            hotkey: Some("V".into()),
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.hotkey.as_deref(), Some("V"));
        assert_eq!(settings.hotkey(), Some(egui::Key::V));
    }

    #[test]
    fn switching_display_units_re_presents_the_same_physical_volume() {
        use benchup_core::LengthUnit;
        let settings = Settings::default();

        // The default 256 mm cube reads as millimetres, centimetres and feet.
        assert!((settings.axis_in(0, LengthUnit::Millimeters) - 256.).abs() < 1e-9);
        assert!((settings.axis_in(0, LengthUnit::Centimeters) - 25.6).abs() < 1e-9);
        assert!((settings.axis_in(2, LengthUnit::Millimeters) - 256.).abs() < 1e-9);
        let feet = settings.axis_in(0, LengthUnit::Feet);
        assert!((feet - 0.256 / 0.3048).abs() < 1e-12, "got {feet} ft");

        // What the user types in one unit lands as the same physical size.
        let mut settings = Settings::default();
        settings.set_axis_from(0, 1., LengthUnit::Feet);
        assert!((settings.size_m[0] - 0.3048).abs() < 1e-12);
        assert!((settings.axis_in(0, LengthUnit::Millimeters) - 304.8).abs() < 1e-9);

        // Switching units must not change the stored value.
        let before = settings.size_m;
        let _ = settings.axis_in(0, LengthUnit::Inches);
        assert_eq!(settings.size_m, before);
    }

    #[test]
    fn toggle_flips_visibility_only() {
        let mut settings = Settings::default();
        settings.toggle();
        assert!(settings.visible);
        settings.toggle();
        assert!(!settings.visible);
        assert!(settings.show_button, "toggling never changes the button preference");
    }
    #[test]
    fn hiding_the_button_hides_the_box() {
        // Reported bug: turning the setting off left the box on screen with no
        // visible control to dismiss it.
        let mut settings = Settings {
            show_button: true,
            visible: true,
            ..Default::default()
        };
        settings.show_button = false;
        settings.enforce();
        assert!(!settings.visible);
    }

    #[test]
    fn enforce_leaves_a_shown_box_visible() {
        let mut settings = Settings {
            show_button: true,
            visible: true,
            ..Default::default()
        };
        settings.enforce();
        assert!(settings.visible, "a shown button never clears the box");
    }

    #[test]
    fn enforce_preserves_size_and_shortcut() {
        let mut settings = Settings {
            show_button: true,
            visible: true,
            size_m: [0.3, 0.2, 0.1],
            hotkey: Some("V".into()),
            ..Default::default()
        };
        settings.show_button = false;
        settings.enforce();
        assert_eq!(settings.size_m, [0.3, 0.2, 0.1]);
        assert_eq!(settings.hotkey.as_deref(), Some("V"), "the shortcut is independent");
    }

    #[test]
    fn hiding_the_button_hides_the_box_even_with_a_shortcut() {
        let mut settings = Settings {
            show_button: true,
            visible: true,
            hotkey: Some("V".into()),
            ..Default::default()
        };
        settings.show_button = false;
        settings.enforce();
        assert!(
            !settings.visible,
            "turning the setting off always hides the box"
        );

        // The shortcut remains a working control afterwards.
        settings.toggle();
        assert!(settings.visible);
    }

    #[test]
    fn bounds_follow_the_configured_size() {
        for size in [[0.1, 0.2, 0.3], [1., 1., 1.], [0.256, 0.256, 0.256]] {
            let settings = Settings {
                size_m: size,
                ..Default::default()
            };
            let (min, max) = settings.bounds();
            assert!((min.x + size[0] / 2.).abs() < 1e-12);
            assert!((max.x - size[0] / 2.).abs() < 1e-12);
            assert!((min.y + size[1] / 2.).abs() < 1e-12);
            assert!((max.y - size[1] / 2.).abs() < 1e-12);
            assert_eq!(min.z, 0.);
            assert!((max.z - size[2]).abs() < 1e-12);
        }
    }

    #[test]
    fn an_empty_model_never_exceeds_the_volume() {
        let settings = Settings {
            size_m: [0.001, 0.001, 0.001],
            ..Default::default()
        };
        assert!(!settings.exceeds(&Model::default()));
    }

}
