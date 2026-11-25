use super::*;

/// A user-defined reference volume drawn as a locked wireframe in the viewport.
///
/// The volume is a global preference, not document geometry, so the same space
/// can be checked across projects. It cannot be selected, edited, or moved in the viewport.
/// Limits in metres, from small parts up to room-sized reference spaces.
pub(super) const MIN_M: f64 = 0.001;
pub(super) const MAX_M: f64 = 100.;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Entry {
    pub id: u64,
    pub name: String,
    /// Width (X), depth (Y), height (Z), always stored in metres.
    pub size_m: [f64; 3],
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            id: 1,
            name: String::new(),
            size_m: [0.256; 3],
        }
    }
}

impl Entry {
    fn axis_in(&self, axis: usize, unit: benchup_core::LengthUnit) -> f64 {
        self.size_m[axis] / unit.factor()
    }

    fn set_axis_from(&mut self, axis: usize, value: f64, unit: benchup_core::LengthUnit) {
        self.size_m[axis] = value * unit.factor();
    }

    fn display_name(&self, index: usize) -> String {
        if self.name.trim().is_empty() {
            tr!("reference_volume.entry", number = index + 1).into()
        } else {
            self.name.clone()
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "SavedSettings")]
pub(super) struct Settings {
    pub visible: bool,
    pub entries: Vec<Entry>,
    /// Retain the last selection while hidden, so the shortcut can restore it.
    pub selected: Option<u64>,
    pub next_id: u64,
    pub hotkey: Option<String>,
}

// Read older single-volume preferences without recreating entries when the user
// deliberately saves an empty catalog. Serialization only writes the new schema.
#[derive(serde::Deserialize)]
#[serde(default)]
struct SavedSettings {
    visible: bool,
    size_m: [f64; 3],
    #[serde(deserialize_with = "deserialize_entries")]
    entries: Option<Vec<Entry>>,
    selected: Option<u64>,
    next_id: u64,
    hotkey: Option<String>,
}

fn deserialize_entries<'de, D>(deserializer: D) -> Result<Option<Vec<Entry>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Settings writes a plain list. Only an absent field means legacy settings;
    // a present empty list must stay empty. RON requires explicit Option syntax,
    // so deserializing Option<Vec<Entry>> directly rejects our own saved lists.
    <Vec<Entry> as serde::Deserialize>::deserialize(deserializer).map(Some)
}

impl Default for SavedSettings {
    fn default() -> Self {
        Self {
            visible: false,
            size_m: [0.256; 3],
            entries: None,
            selected: Some(1),
            next_id: 2,
            hotkey: None,
        }
    }
}

impl From<SavedSettings> for Settings {
    fn from(saved: SavedSettings) -> Self {
        let mut settings = Self {
            visible: saved.visible,
            entries: saved.entries.unwrap_or_else(|| {
                vec![Entry {
                    size_m: saved.size_m,
                    ..Entry::default()
                }]
            }),
            selected: saved.selected,
            next_id: saved.next_id,
            hotkey: saved.hotkey,
        };
        settings.sanitize();
        settings
    }
}

impl Default for Settings {
    fn default() -> Self {
        SavedSettings::default().into()
    }
}

impl Settings {
    pub(super) fn sanitize(&mut self) {
        let mut used = std::collections::HashSet::new();
        let mut next = self.next_id.max(2);
        for entry in &mut self.entries {
            if entry.id == 0 || used.contains(&entry.id) {
                while used.contains(&next) || next == 0 {
                    next = next.wrapping_add(1);
                }
                entry.id = next;
            }
            used.insert(entry.id);
            next = next.max(entry.id.saturating_add(1));
            for value in &mut entry.size_m {
                if !value.is_finite() {
                    *value = 0.256;
                }
                *value = value.clamp(MIN_M, MAX_M);
            }
        }
        self.next_id = next;
        if !self.selected.is_some_and(|id| used.contains(&id)) {
            self.selected = self.entries.first().map(|entry| entry.id);
        }
        if self.entries.is_empty() {
            self.visible = false;
        }
        if let Some(key) = &self.hotkey
            && !super::shortcut_input::valid_tool_key(key)
        {
            self.hotkey = None;
        }
    }

    pub(super) fn add_entry(&mut self) {
        let mut id = self.next_id;
        while id == 0 || self.entries.iter().any(|entry| entry.id == id) {
            id = id.wrapping_add(1);
        }
        self.entries.push(Entry {
            id,
            ..Entry::default()
        });
        self.next_id = id.wrapping_add(1);
        if self.selected.is_none() {
            self.selected = Some(id);
        }
    }

    pub(super) fn remove_entry(&mut self, id: u64) {
        self.entries.retain(|entry| entry.id != id);
        if self.selected == Some(id) {
            self.visible = false;
            self.selected = self.entries.first().map(|entry| entry.id);
        }
    }

    pub(super) fn toggle_entry(&mut self, id: u64) {
        if self.entries.iter().any(|entry| entry.id == id) {
            self.visible = !(self.visible && self.selected == Some(id));
            self.selected = Some(id);
        }
    }

    pub(super) fn toggle(&mut self) {
        if let Some(id) = self.selected {
            self.toggle_entry(id);
        }
    }

    pub(super) fn size_m(&self) -> [f64; 3] {
        self.entries
            .iter()
            .find(|entry| Some(entry.id) == self.selected)
            .map_or([0.256; 3], |entry| entry.size_m)
    }

    pub(super) fn bounds(&self) -> (DVec3, DVec3) {
        let size = self.size_m();
        let half = DVec3::from_array(size) * 0.5;
        (
            DVec3::new(-half.x, -half.y, 0.),
            DVec3::new(half.x, half.y, size[2]),
        )
    }

    pub(super) fn contains(&self, low: DVec3, high: DVec3) -> bool {
        let (min, max) = self.bounds();
        let tolerance = 1e-9;
        (0..3).all(|axis| low[axis] >= min[axis] - tolerance && high[axis] <= max[axis] + tolerance)
    }

    pub(super) fn exceeds(&self, model: &Model) -> bool {
        model
            .bounds()
            .is_some_and(|(low, high)| !self.contains(low, high))
    }

    pub(super) fn hotkey(&self) -> Option<egui::Key> {
        self.hotkey.as_deref().and_then(egui::Key::from_name)
    }

    /// The shared reference-mesh API consumes projected strips, so preparation
    /// is bounded to twelve segments and reused on unchanged frames. Keeping
    /// these tiny references separate leaves the model's GPU buffers resident.
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
            mesh.reference_line(corners[a], corners[b], 1.5, ACCENT, true);
        }
    }
}

impl BenchUpApp {
    pub(super) fn reference_volume_menu(&mut self, ui: &mut egui::Ui) {
        let button = egui::Button::new((
            Self::menu_item_icon(ui, Some("reference_volume")),
            tr!("settings.printer_and_tool_volume"),
        ))
        .gap(ui.spacing().icon_spacing)
        .right_text(egui::menu::SubMenuButton::arrow_atom(None));
        egui::menu::SubMenuButton::from_button(button)
            .config(MenuPress::submenu_config(ui))
            .ui(ui, |ui| {
                Self::menu_style(ui.style_mut());
                ui.set_width(264.);
                self.reference_volume_controls(ui);
            })
            .0
            .menu_press_target();
    }

    pub(super) fn reference_volume_controls(&mut self, ui: &mut egui::Ui) {
        let entries: Vec<_> = self
            .general_settings
            .volume
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.id, entry.display_name(index)))
            .collect();
        if entries.is_empty() {
            ui.text_weak(tr!("reference_volume.empty"));
        }
        for (id, name) in entries {
            let mut active = self.general_settings.volume.visible
                && self.general_settings.volume.selected == Some(id);
            if Self::menu_checkbox(ui, &mut active, &name).changed() {
                self.general_settings.volume.toggle_entry(id);
                self.frame_reference_volume(ui.ctx());
            }
        }
    }

    pub(super) fn reference_volume_settings(&mut self, ui: &mut egui::Ui) {
        use crate::inspector_table::{Column, InspectorTable};
        ui.separator();
        ui.text_heading(tr!("settings.printer_and_tool_volume"));
        ui.text_weak(tr!("reference_volume.settings_hint"));
        let units = self.doc.display.units;
        let length = units.length;
        let factor = length.factor();
        let decimals = units.precision.length.unwrap_or(3) as usize;
        let remove_width = ui
            .painter()
            .layout_no_wrap(
                tr!("common.remove").into(),
                egui::TextStyle::Button.resolve(ui.style()),
                ui.visuals().text_color(),
            )
            .size()
            .x
            + 2. * ui.spacing().button_padding.x
            + 4.;
        let key = "settings/reference_volumes";
        let scroll = sidebar::restored_scroll_area(
            ui,
            key,
            egui::ScrollArea::horizontal()
                .id_salt(key)
                .auto_shrink([false, true]),
        );
        let mut remove = None;
        let output = scroll.show(ui, |ui| {
            let mut table = InspectorTable::new(
                ui.available_width(),
                [
                    Column::at_least(150.),
                    Column::at_least(110.),
                    Column::at_least(110.),
                    Column::at_least(110.),
                    Column::fixed(remove_width),
                ],
            );
            table.header(
                ui,
                [
                    tr!("common.name"),
                    tr!("settings.width_x"),
                    tr!("settings.depth_y"),
                    tr!("settings.height_z"),
                    "",
                ],
            );
            for (index, entry) in self.general_settings.volume.entries.iter_mut().enumerate() {
                let hint = entry.display_name(index);
                table.row(
                    ui,
                    ("reference_volume_entry", entry.id),
                    |column, cell| match column {
                        0 => {
                            cell.add(
                                egui::TextEdit::singleline(&mut entry.name)
                                    .hint_text(hint.as_str()),
                            );
                        }
                        1..=3 => {
                            let axis = column - 1;
                            let mut value = entry.axis_in(axis, length);
                            if cell
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .speed(1.)
                                        .max_decimals(decimals)
                                        .range(MIN_M / factor..=MAX_M / factor)
                                        .suffix(format!(" {}", length.symbol())),
                                )
                                .changed()
                            {
                                entry.set_axis_from(axis, value, length);
                            }
                        }
                        _ => {
                            if cell.button(tr!("common.remove")).clicked() {
                                remove = Some(entry.id);
                            }
                        }
                    },
                );
            }
        });
        sidebar::remember_scroll(ui.ctx(), key, output.state.clamped_offset());
        if let Some(id) = remove {
            self.general_settings.volume.remove_entry(id);
        }
        if ui.button(tr!("reference_volume.add")).clicked() {
            self.general_settings.volume.add_entry();
        }
    }

    pub(super) fn reference_volume_shortcut(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let assigned = self.general_settings.volume.hotkey.is_some();
            let shown = self
                .general_settings
                .volume
                .hotkey
                .clone()
                .unwrap_or_else(|| tr!("common.none").into());
            let width = (ui.available_width() - if assigned { 32. } else { 0. }).max(24.);
            if let Some(key) =
                super::shortcut_input::shortcut_input(ui, "settings_volume_key", &shown, width)
            {
                self.general_settings.volume.hotkey = Some(key);
            }
            if assigned {
                let clear = ui
                    .small_button("✕")
                    .on_hover_text(tr!("settings.clear_the_toggle_shortcut"));
                clear.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        tr!("settings.clear_the_toggle_shortcut"),
                    )
                });
                if clear.clicked() {
                    self.general_settings.volume.hotkey = None;
                }
            }
        });
    }

    /// Toggling on frames the reference, which can be tiny at the current zoom.
    /// Toggling off leaves the camera untouched.
    pub(super) fn toggle_printer_volume(&mut self, ctx: &egui::Context) {
        self.general_settings.volume.toggle();
        self.frame_reference_volume(ctx);
    }

    fn frame_reference_volume(&mut self, ctx: &egui::Context) {
        if self.general_settings.volume.visible {
            self.doc.camera.fit(
                Some(self.general_settings.volume.bounds()),
                dv(self.viewport.size().max(Vec2::splat(1.))),
            );
            self.camera_changed(ctx);
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
        let size = self.general_settings.volume.size_m();
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
            (rect.width() - 106.).clamp(160., 494.),
        );
        let banner = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 30.),
            egui::vec2(galley.size().x + 50., galley.size().y.max(18.) + 16.),
        );
        painter.rect_filled(banner, 7, paper);
        painter.rect_stroke(
            banner,
            7,
            Stroke::new(1., ink.gamma_multiply(0.5)),
            egui::StrokeKind::Inside,
        );
        icons::line_icon(
            painter,
            Pos2::new(banner.left() + 21., banner.center().y),
            "warning",
            Color32::from_rgb(245, 155, 45),
        );
        let text_top = banner.center().y - galley.size().y * 0.5;
        painter.galley(Pos2::new(banner.left() + 38., text_top), galley, ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_hidden_with_an_unassigned_shortcut() {
        let settings = Settings::default();
        assert!(!settings.visible);
        assert_eq!(settings.hotkey, None);
        assert_eq!(settings.size_m(), [0.256, 0.256, 0.256]);
    }

    #[test]
    fn bottom_face_center_sits_on_the_origin() {
        let settings = Settings {
            entries: vec![Entry {
                size_m: [0.2, 0.1, 0.05],
                ..Default::default()
            }],
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
            entries: vec![Entry {
                size_m: [0.1, 0.1, 0.1],
                ..Default::default()
            }],
            ..Default::default()
        };
        let (min, max) = settings.bounds();
        assert!(settings.contains(min, max));
        assert!(settings.contains(min, max + DVec3::splat(1e-12)));
    }

    #[test]
    fn geometry_outside_the_volume_is_detected() {
        let settings = Settings {
            entries: vec![Entry {
                size_m: [0.1, 0.1, 0.1],
                ..Default::default()
            }],
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
            entries: vec![Entry {
                size_m: [f64::NAN, 0.01, 0.01],
                ..Default::default()
            }],
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(
            settings.size_m(),
            [Settings::default().size_m()[0], 0.01, 0.01]
        );

        let mut settings = Settings {
            entries: vec![Entry {
                size_m: [-5., 1e9, 0.05],
                ..Default::default()
            }],
            hotkey: Some("nope".into()),
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.size_m(), [MIN_M, MAX_M, 0.05]);
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
        assert!((settings.entries[0].axis_in(0, LengthUnit::Millimeters) - 256.).abs() < 1e-9);
        assert!((settings.entries[0].axis_in(0, LengthUnit::Centimeters) - 25.6).abs() < 1e-9);
        assert!((settings.entries[0].axis_in(2, LengthUnit::Millimeters) - 256.).abs() < 1e-9);
        let feet = settings.entries[0].axis_in(0, LengthUnit::Feet);
        assert!((feet - 0.256 / 0.3048).abs() < 1e-12, "got {feet} ft");

        // What the user types in one unit lands as the same physical size.
        let mut settings = Settings::default();
        settings.entries[0].set_axis_from(0, 1., LengthUnit::Feet);
        assert!((settings.size_m()[0] - 0.3048).abs() < 1e-12);
        assert!((settings.entries[0].axis_in(0, LengthUnit::Millimeters) - 304.8).abs() < 1e-9);

        // Switching units must not change the stored value.
        let before = settings.size_m();
        let _ = settings.entries[0].axis_in(0, LengthUnit::Inches);
        assert_eq!(settings.size_m(), before);
    }

    #[test]
    fn toggle_flips_visibility_only() {
        let mut settings = Settings::default();
        settings.toggle();
        assert!(settings.visible);
        settings.toggle();
        assert!(!settings.visible);
    }
    #[test]
    fn bounds_follow_the_configured_size() {
        for size in [[0.1, 0.2, 0.3], [1., 1., 1.], [0.256, 0.256, 0.256]] {
            let settings = Settings {
                entries: vec![Entry {
                    size_m: size,
                    ..Default::default()
                }],
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
            entries: vec![Entry {
                size_m: [0.001, 0.001, 0.001],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(!settings.exceeds(&Model::default()));
    }

    #[test]
    fn selecting_entries_is_exclusive_and_shortcut_restores_last_selection() {
        let mut settings = Settings::default();
        settings.add_entry();
        let second = settings.entries[1].id;
        settings.entries[1].size_m = [1., 2., 3.];
        settings.toggle_entry(1);
        assert!(settings.visible);
        settings.toggle_entry(second);
        assert!(settings.visible);
        assert_eq!(settings.selected, Some(second));
        assert_eq!(settings.size_m(), [1., 2., 3.]);
        settings.toggle_entry(second);
        assert!(!settings.visible);
        settings.toggle();
        assert!(settings.visible);
        assert_eq!(settings.selected, Some(second));
        settings.remove_entry(1);
        assert!(
            settings.visible,
            "removing an inactive entry preserves the active volume"
        );
        settings.remove_entry(second);
        settings.toggle();
        assert!(!settings.visible);
        assert_eq!(settings.selected, None);
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            restored, settings,
            "empty catalogs remain empty after reload"
        );
        settings.add_entry();
        assert_ne!(settings.entries[0].id, second, "deleted IDs are not reused");
    }

    #[test]
    fn damaged_catalog_ids_and_selection_are_repaired() {
        let mut settings = Settings::default();
        settings.entries.push(Entry::default());
        settings.entries.push(Entry {
            id: 0,
            ..Default::default()
        });
        settings.selected = Some(99);
        settings.sanitize();
        let ids: std::collections::HashSet<_> =
            settings.entries.iter().map(|entry| entry.id).collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&0));
        assert_eq!(settings.selected, Some(settings.entries[0].id));
        settings.add_entry();
        assert_eq!(settings.entries.len(), 4);
        assert!(!ids.contains(&settings.entries[3].id));
    }
}
