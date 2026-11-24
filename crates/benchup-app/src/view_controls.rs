use benchup_core::tr;
use super::*;

impl BenchUpApp {
    pub(super) fn view_contents(&mut self, ui: &mut egui::Ui) {
        let before = self.doc.camera.clone();
        inspector_section(ui, tr!("view_controls.camera"), "view_camera", false, |ui| {
            ui.text_label(tr!("view_controls.projection"));
            let button_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
            ui.horizontal(|ui| {
                for (projection, label) in [
                    (Projection::Perspective, tr!("view_controls.perspective")),
                    (Projection::Parallel, tr!("view_controls.parallel")),
                ] {
                    let response = ui.add_sized(
                        [button_width, ui.spacing().interact_size.y],
                        egui::Button::new(label).selected(self.doc.camera.projection == projection),
                    );
                    if response.clicked() {
                        self.doc.camera.projection = projection;
                    }
                    if projection == Projection::Parallel {
                        response
                            .on_hover_text(tr!("view_controls.orthographic_projection_parallel_lines_stay_parallel"));
                    }
                }
            });
            ui.add_enabled_ui(
                self.doc.camera.projection == Projection::Perspective,
                |ui| {
                    ui.text_label(tr!("view_controls.field_of_view"));
                    let mut degrees = self.doc.camera.fov.to_degrees();
                    if ui
                        .add(egui::Slider::new(&mut degrees, 15. ..=100.).suffix(tr!("view_controls.degree_symbol")))
                        .changed()
                    {
                        self.doc.camera.fov = degrees.to_radians();
                    }
                },
            );
            let button_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [button_width, ui.spacing().interact_size.y],
                        egui::Button::new(tr!("view_controls.fit_model")),
                    )
                    .clicked()
                {
                    self.doc.camera.fit(
                        self.doc.model.bounds(),
                        dv(self.viewport.size().max(Vec2::splat(1.))),
                    );
                }
                ui.add_enabled_ui(!self.selected.is_empty(), |ui| {
                    if ui
                        .add_sized(
                            [button_width, ui.spacing().interact_size.y],
                            egui::Button::new(tr!("view_controls.fit_selection")),
                        )
                        .clicked()
                    {
                        self.fit_selection(ui.ctx());
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [button_width, ui.spacing().interact_size.y],
                        egui::Button::new(tr!("view_controls.reset_camera")),
                    )
                    .clicked()
                {
                    self.doc.camera = Camera::default();
                    self.doc.camera.fit(
                        self.doc.model.bounds(),
                        dv(self.viewport.size().max(Vec2::splat(1.))),
                    );
                }
            });
        });
        if before != self.doc.camera {
            self.changed(ui.ctx());
        }
    }
}

impl BenchUpApp {
    pub(in crate::app) fn paint_menu(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::for_ui(ui);
        let color = Color32::from_rgb(
            self.paint_color[0],
            self.paint_color[1],
            self.paint_color[2],
        );
        let icon = egui::Atom::paint(egui::vec2(18., 18.), |ui, args| {
            icons::paint_bucket(ui.painter(), args.rect, color);
        });
        let response = ui
            .add(
                egui::Button::new((icon, icons::dropdown_atom(colors.muted)))
                .min_size(egui::vec2(0., 32.))
                .selected(self.tool == Tool::Paint),
            )
            .on_hover_text(tr!("view_controls.choose_how_to_paint_and_a_paint_color"));
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr!("view_controls.color")));
        egui::Popup::menu(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                Self::menu_style(ui.style_mut());
                ui.set_width(256.);
                let padding = ui.spacing().button_padding.x as i8;
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(padding, 6))
                    .show(ui, |ui| {
                        ui.text_label(tr!("view_controls.paint_by"));
                        let button_width =
                            (ui.available_width() - ui.spacing().item_spacing.x) / 2.;
                        ui.horizontal(|ui| {
                            for (target, label) in
                                [(PaintTarget::Face, tr!("view_controls.face")), (PaintTarget::Group, tr!("view_controls.group"))]
                            {
                                if ui
                                    .add_sized(
                                        [button_width, 30.],
                                        egui::Button::new(label)
                                            .selected(self.paint_target == target),
                                    )
                                    .clicked()
                                {
                                    self.paint_target = target;
                                    self.set_tool(Tool::Paint);
                                }
                            }
                        });
                        ui.add_space(4.);
                        ui.separator();
                        ui.add_space(4.);
                        self.paint_color_controls(ui, true);
                    });
            });
    }

    pub(in crate::app) fn paint_color_controls(&mut self, ui: &mut egui::Ui, activate_paint: bool) {
        crate::inspector_table::inspector_field(ui, "paint_color", tr!("view_controls.choose_color"), |ui| {
            ui.spacing_mut().interact_size.x = ui.available_width();
            let response = ui.color_edit_button_srgb(&mut self.paint_color);
            if response.changed() && activate_paint {
                self.set_tool(Tool::Paint);
            }
            response
        });
        ui.add_space(6.);
        ui.text_label(
            RichText::new(tr!("view_controls.preset_colors"))
                .small()
                .color(Palette::for_ui(ui).muted),
        );
        let presets = [
            // Light hues, from warm to cool.
            [245, 182, 186],
            [250, 205, 160],
            [249, 235, 162],
            [186, 216, 172],
            [175, 204, 238],
            [217, 188, 231],
            // Saturated hues in the same order.
            [210, 48, 48],
            [239, 128, 45],
            [245, 210, 65],
            [60, 153, 88],
            [55, 105, 195],
            [133, 72, 181],
            // Muted hues in the same order.
            [195, 96, 67],
            [194, 132, 75],
            [225, 179, 87],
            [108, 148, 102],
            [95, 143, 177],
            [163, 121, 167],
            // Neutrals and wood tones, from light to dark.
            [255, 255, 255],
            [236, 231, 215],
            [201, 204, 207],
            [139, 145, 151],
            [148, 101, 65],
            [78, 84, 81],
        ];
        use crate::inspector_table::{Column, InspectorTable};
        let mut table = InspectorTable::new(ui.available_width(), [Column::flex(1.); 6]);
        for (row_index, row) in presets.chunks(6).enumerate() {
            if row_index > 0 {
                ui.add_space(4.);
            }
            table.row(ui, ("paint_preset_row", row_index), |column, cell| {
                let color = row[column];
                let rgb = Color32::from_rgb(color[0], color[1], color[2]);
                // Let narrow cells size the swatch instead of the default button minimum.
                cell.spacing_mut().interact_size.x = cell.available_width();
                cell.spacing_mut().button_padding.x = 0.;
                if cell
                    .add(egui::Button::new("").fill(rgb))
                    .on_hover_text(format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2]))
                    .clicked()
                {
                    self.paint_color = color;
                    if activate_paint {
                        self.set_tool(Tool::Paint);
                    }
                }
            });
        }
    }

    pub(in crate::app) fn sections_menu(&mut self, ui: &mut egui::Ui) {
        let before = self.doc.display.clone();
        let button = egui::Button::new((Self::menu_item_icon(ui, Some("sections")), tr!("view_controls.sections")))
            .gap(ui.spacing().icon_spacing)
            .right_text(egui::menu::SubMenuButton::arrow_atom(None));
        egui::menu::SubMenuButton::from_button(button)
            .config(
                egui::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                Self::menu_style(ui.style_mut());
                ui.set_width(208.);
                let d = &mut self.doc.display;
                Self::menu_checkbox(ui, &mut d.section_cuts, tr!("view_controls.section_cuts"));
                Self::menu_checkbox(ui, &mut d.section_fill, tr!("view_controls.section_fill"));
                ui.separator();
                let padding = ui.spacing().button_padding.x as i8;
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(padding, 0))
                    .show(ui, |ui| {
                        use crate::inspector_table::{Column, InspectorTable};
                        let mut table = InspectorTable::new(
                            ui.available_width(),
                            [Column::fixed(112.), Column::flex(1.)],
                        );
                        for (axis, label) in [tr!("view_controls.x_vertical"), tr!("view_controls.y_vertical"), tr!("view_controls.z_horizontal")]
                            .into_iter()
                            .enumerate()
                        {
                            table.row(ui, ("section_axis", axis), |column, cell| {
                                if column == 0 {
                                    if cell
                                        .checkbox(&mut d.section_axes[axis], label)
                                        .on_hover_text(
                                            tr!("view_controls.keeps_geometry_on_the_negative_side_of_this"),
                                        )
                                        .changed()
                                    {
                                        d.section_planes =
                                            d.section_axes.iter().any(|&enabled| enabled);
                                    }
                                } else {
                                    let offset = if axis == 2 {
                                        &mut d.section_height
                                    } else {
                                        &mut d.section_offsets[axis]
                                    };
                                    cell.add(
                                        egui::DragValue::new(offset)
                                            .speed(0.05)
                                            .range(-1e8..=1e8)
                                            .suffix(" m"),
                                    );
                                }
                            });
                        }
                    });
            })
            .0
            .on_hover_text(tr!("view_controls.configure_section_planes_cuts_fill_and_axis_positions"));
        if self.doc.display != before {
            self.changed(ui.ctx());
        }
    }
}
