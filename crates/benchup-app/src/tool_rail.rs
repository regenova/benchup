use super::*;

const BUTTON_SIZE: Vec2 = egui::vec2(64., 54.);
const SEPARATOR_WIDTH: f32 = 12.;
const EDGE_GAP: f32 = 16.;
const FRAME_INSET: f32 = 7.;
const SCROLLBAR_HEIGHT: f32 = 6.;

// Separate creation and navigation from modeling, then separate the utility tools.
pub(super) const DEFAULT_SEPARATORS: [usize; 2] = [4, 10];

impl BenchUpApp {
    fn tool_rail_width(&self) -> f32 {
        BUTTON_SIZE.x * self.tool_rail_order.len() as f32
            + SEPARATOR_WIDTH * self.tool_rail_separators.len() as f32
            + FRAME_INSET * 2.
    }

    pub(super) fn tool_rail_rect(&self, viewport: Rect) -> Option<Rect> {
        let width = self.tool_rail_width().min(viewport.width() - EDGE_GAP * 2.);
        if width < BUTTON_SIZE.x + FRAME_INSET * 2. || viewport.height() < 100. {
            return None;
        }
        let scroll_height = if width < self.tool_rail_width() {
            SCROLLBAR_HEIGHT
        } else {
            0.
        };
        let height = BUTTON_SIZE.y + FRAME_INSET * 2. + scroll_height;
        Some(Rect::from_center_size(
            viewport.center_bottom() - egui::vec2(0., EDGE_GAP + height * 0.5),
            egui::vec2(width, height),
        ))
    }

    pub(super) fn tool_rail(&mut self, ui: &mut egui::Ui, viewport: Rect) -> Option<Rect> {
        let rect = self.tool_rail_rect(viewport)?;
        let colors = Palette::for_ui(ui);
        let dark = ui.visuals().dark_mode;
        let overflow = rect.width() < self.tool_rail_width();
        let enabled = ui.is_enabled() && self.gesture.is_none() && self.section_drag.is_none();
        let area = egui::Area::new(egui::Id::unique("floating_tool_rail"))
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .default_size(rect.size())
            .constrain_to(viewport)
            .constrain(false)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                if !enabled {
                    ui.visuals_mut().disabled_alpha = 1.;
                    ui.disable();
                }
                let context_id = ui.make_persistent_id("tool_rail_context_menu");
                let context_open = egui::Popup::is_id_open(ui.ctx(), context_id);
                let secondary_down = ui.input(|input| input.pointer.secondary_down());
                let response = egui::Frame::new()
                    .fill(colors.surface)
                    .stroke(Stroke::new(1., colors.border))
                    .inner_margin(6)
                    .corner_radius(14)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 18,
                        spread: 0,
                        color: Color32::from_black_alpha(if dark { 80 } else { 32 }),
                    })
                    .show(ui, |ui| {
                        let width = rect.width() - FRAME_INSET * 2.;
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        let mut draw_tools = |ui: &mut egui::Ui| {
                            ui.horizontal(|ui| {
                                self.tool_rail_contents(
                                    ui,
                                    rect,
                                    overflow,
                                    context_open || secondary_down,
                                )
                            })
                            .inner
                        };
                        if !overflow {
                            return draw_tools(ui);
                        }
                        let scroll_style = &mut ui.spacing_mut().scroll;
                        scroll_style.floating = false;
                        scroll_style.bar_width = 4.;
                        scroll_style.bar_inner_margin = 2.;
                        scroll_style.bar_outer_margin = 0.;
                        ui.style_mut().always_scroll_the_only_direction = true;
                        let scroll = sidebar::restored_scroll_area(
                            ui,
                            "sidebar_left",
                            egui::ScrollArea::horizontal()
                                .id_salt("tool_scroll")
                                .max_width(width)
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                                ),
                        )
                        .show(ui, draw_tools);
                        sidebar::remember_scroll(
                            ui.ctx(),
                            "sidebar_left",
                            scroll.state.clamped_offset(),
                        );
                        scroll.inner
                    });
                let (tools, separators) = response.inner;
                let frame = response.response;
                let over_rail = enabled
                    && ui.ctx().rect_contains_pointer(frame.layer_id, frame.rect)
                    && !egui::DragAndDrop::has_any_payload(ui.ctx());
                if over_rail && !overflow && !context_open {
                    self.cycle_rail_tools(ui);
                }
                let mut open_context = false;
                if over_rail
                    && ui.input(|input| input.pointer.secondary_clicked())
                    && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
                {
                    // Save the boundary so moving to the menu cannot change its target.
                    let target = separators
                        .iter()
                        .find(|(_, rect)| rect.x_range().contains(pointer.x))
                        .map(|&(index, _)| (true, index))
                        .unwrap_or_else(|| {
                            (
                                false,
                                tools
                                    .iter()
                                    .position(|rect| pointer.x < rect.left())
                                    .unwrap_or(tools.len()),
                            )
                        });
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(context_id, target));
                    egui::Popup::close_all(ui.ctx());
                    open_context = true;
                }
                egui::Popup::menu(&frame)
                    .id(context_id)
                    .open_memory(open_context.then_some(egui::SetOpenCommand::Bool(true)))
                    .at_pointer_fixed()
                    .show(|ui| {
                        if let Some((remove, index)) = ui
                            .ctx()
                            .data(|data| data.get_temp::<(bool, usize)>(context_id))
                        {
                            let label = if remove {
                                tr!("tools.remove_separator")
                            } else {
                                tr!("tools.add_separator")
                            };
                            let available = remove || !self.tool_rail_separators.contains(&index);
                            if ui
                                .add_enabled(available, egui::Button::new(label))
                                .clicked()
                            {
                                if remove {
                                    self.tool_rail_separators.retain(|&slot| slot != index);
                                } else {
                                    self.tool_rail_separators.push(index);
                                    self.tool_rail_separators.sort_unstable();
                                }
                                ui.close();
                            }
                        }
                    });
                frame.rect
            });
        Some(area.inner)
    }

    pub(super) fn show_tool_hint(&self, ui: &egui::Ui, viewport: Rect, rail: Rect) {
        let message = self.tool_hint(ui.ctx());
        let width = (viewport.width() - EDGE_GAP * 2. - 24.).min(760.);
        let mut job = egui::text::LayoutJob::simple(
            message.into_owned(),
            FontId::proportional(12.),
            Color32::from_rgb(245, 245, 245),
            width,
        );
        job.wrap.max_rows = 3;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let size = galley.size() + egui::vec2(24., 10.);
        let corner = egui::pos2(rail.center().x - size.x * 0.5, rail.top() - 8. - size.y);
        if corner.y < viewport.top() {
            return;
        }
        egui::Area::new(egui::Id::unique("floating_tool_hint"))
            .order(egui::Order::Foreground)
            .fixed_pos(corner)
            .default_size(size)
            .constrain_to(viewport)
            .constrain(false)
            .fade_in(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(59, 63, 65))
                    .inner_margin(egui::Margin::symmetric(12, 5))
                    .corner_radius(16)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(galley).selectable(false));
                    });
            });
    }

    fn tool_rail_contents(
        &mut self,
        ui: &mut egui::Ui,
        rail: Rect,
        overflow: bool,
        suppress_flyouts: bool,
    ) -> (Vec<Rect>, Vec<(usize, Rect)>) {
        let colors = Palette::for_ui(ui);
        let selection_shape = ui.painter().add(egui::Shape::Noop);
        let content_left = ui.cursor().left();
        let dragged = egui::DragAndDrop::payload::<Tool>(ui.ctx());
        let dragged_separator = egui::DragAndDrop::payload::<RailSeparatorDrag>(ui.ctx());
        let mut preview_order = self.tool_rail_order.clone();
        let mut preview_separators = self.tool_rail_separators.clone();
        let mut separator_target = dragged_separator.as_ref().map(|dragged| dragged.0);
        if ui.is_enabled() && (dragged.is_some() || dragged_separator.is_some()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let visible = rail.intersect(ui.clip_rect().expand(FRAME_INSET));
            if let Some(pointer) = ui.input(|input| input.pointer.hover_pos())
                && visible.contains(pointer)
            {
                let mut left = content_left;
                let insertion = (0..preview_order.len())
                    .find(|index| {
                        if self.tool_rail_separators.contains(index) {
                            left += SEPARATOR_WIDTH;
                        }
                        let before_center = pointer.x < left + BUTTON_SIZE.x * 0.5;
                        left += BUTTON_SIZE.x;
                        before_center
                    })
                    .unwrap_or(preview_order.len());
                if let Some(&tool) = dragged.as_deref() {
                    reorder_tool(&mut preview_order, tool, insertion);
                }
                if let Some(source) = separator_target
                    && move_separator(&mut preview_separators, source, insertion)
                {
                    separator_target = Some(insertion);
                }
                if ui.input(|input| input.pointer.primary_released()) {
                    if let Some(&tool) = dragged.as_deref() {
                        self.reorder_rail_tool(tool, insertion);
                    }
                    if let Some(source) = &dragged_separator {
                        self.reorder_rail_separator(source.0, insertion);
                    }
                    egui::DragAndDrop::clear_payload(ui.ctx());
                } else if overflow {
                    // Let a held tool or separator cross beyond the visible slots.
                    let edge = 24.;
                    let scroll = (visible.left() + edge - pointer.x).clamp(0., edge)
                        - (pointer.x - (visible.right() - edge)).clamp(0., edge);
                    if scroll != 0. {
                        ui.scroll_with_delta_animation(
                            egui::vec2(scroll * 0.25, 0.),
                            egui::style::ScrollAnimation::none(),
                        );
                        ui.ctx().request_repaint();
                    }
                }
            }
        }

        let mut tools = Vec::with_capacity(preview_order.len());
        let mut separators = Vec::with_capacity(preview_separators.len());
        for index in 0..=preview_order.len() {
            if preview_separators.contains(&index) {
                let source = dragged_separator
                    .as_deref()
                    .filter(|_| separator_target == Some(index));
                let placeholder = source.is_some();
                let slot = source.map_or(index, |dragged| dragged.0);
                let (_, target) = ui.allocate_space(egui::vec2(SEPARATOR_WIDTH, BUTTON_SIZE.y));
                let rect = slide(
                    ui,
                    target,
                    content_left,
                    ("tool_rail_separator_position", slot),
                );
                let response = ui.interact(
                    rect,
                    ui.scope_id().with(("tool_rail_separator", slot)),
                    if placeholder {
                        Sense::hover()
                    } else {
                        Sense::click_and_drag()
                    },
                );
                response.dnd_set_drag_payload(RailSeparatorDrag(slot));
                if response.drag_started() {
                    egui::Popup::close_all(ui.ctx());
                }
                if placeholder {
                    ui.painter().rect_filled(rect, 5, colors.hovered);
                } else if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                ui.painter().vline(
                    rect.center().x,
                    rect.shrink2(egui::vec2(0., 6.)).y_range(),
                    Stroke::new(
                        if placeholder { 2. } else { 1. },
                        if placeholder {
                            colors.accent
                        } else {
                            colors.border
                        },
                    ),
                );
                separators.push((index, rect));
            }
            if let Some(&slot) = preview_order.get(index) {
                let (_, target) = ui.allocate_space(BUTTON_SIZE);
                let rect = slide(
                    ui,
                    target,
                    content_left,
                    ("tool_rail_position", slot as usize),
                );
                let placeholder = dragged.as_deref() == Some(&slot);
                self.tool_rail_button(ui, rect, slot, placeholder, suppress_flyouts);
                tools.push(rect);
            }
        }
        if let Some(index) = preview_order
            .iter()
            .position(|&slot| slot == self.active_rail_slot())
            && dragged.as_deref() != Some(&preview_order[index])
        {
            let rect = slide(ui, tools[index], content_left, "tool_rail_selection");
            ui.painter().set(
                selection_shape,
                egui::Shape::rect_filled(rect, 10, colors.selected),
            );
        }
        (tools, separators)
    }

    fn tool_rail_button(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        slot: Tool,
        placeholder: bool,
        suppress_flyouts: bool,
    ) {
        if !rect.intersects(ui.clip_rect()) {
            // The popup may still be open if scrolling moved its anchor offscreen.
            let id = ui.scope_id().with(("tool_rail_slot", slot as usize));
            egui::Popup::close_id(ui.ctx(), id.with("popup"));
            return;
        }
        let colors = Palette::for_ui(ui);
        let tool = self.tool_for_rail_slot(slot);
        let options: &[Tool] = match slot {
            Tool::Line => &Tool::DRAWING,
            Tool::Dimensions => &Tool::MEASUREMENT_MENU,
            Tool::Union => &Tool::SOLID,
            Tool::FollowMe => &Tool::ADVANCED,
            _ => &[],
        };
        let active = self.active_rail_slot() == slot;
        let response = ui.interact(
            rect,
            ui.scope_id().with(("tool_rail_slot", slot as usize)),
            if placeholder {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            },
        );
        response.dnd_set_drag_payload(slot);
        if response.drag_started() {
            egui::Popup::close_all(ui.ctx());
        }
        let label = match slot {
            Tool::FollowMe => tr!("follow_me.advanced_tools"),
            _ if tool == Tool::TapeMeasure => tr!("app.tape"),
            _ => tool.label(),
        };
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                if options.is_empty() {
                    egui::Role::RadioButton
                } else {
                    egui::Role::ComboBox
                },
                ui.is_enabled(),
                active,
                label,
            )
        });
        if placeholder || !active && (response.hovered() || response.has_focus()) {
            ui.painter().rect_filled(rect, 10, colors.hovered);
        }
        if placeholder {
            ui.painter().rect_stroke(
                rect,
                10,
                Stroke::new(1., colors.accent),
                egui::StrokeKind::Inside,
            );
        }
        let color = if active {
            colors.accent
        } else if ui.visuals().dark_mode {
            colors.muted
        } else {
            Color32::from_rgb(96, 105, 114)
        };
        let icon = egui::pos2(rect.center().x, rect.top() + 17.);
        icons::toolbar_tool(
            ui.painter(),
            Rect::from_center_size(icon, Vec2::splat(21.)),
            tool,
            color,
        );
        let mut job = egui::text::LayoutJob::simple(
            label.to_owned(),
            FontId::proportional(11.),
            color,
            BUTTON_SIZE.x - 6.,
        );
        job.halign = egui::Align::Center;
        job.wrap.max_rows = 2;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(rect.center().x, rect.top() + 39. - galley.size().y * 0.5),
            galley,
            color,
        );
        if !options.is_empty() {
            // Point toward the flyout without taking space from the label.
            let tip = egui::pos2(rect.right() - 9., rect.top() + 10.);
            ui.painter().line(
                vec![tip + egui::vec2(-3., 2.), tip, tip + egui::vec2(3., 2.)],
                Stroke::new(1., color),
            );
            if !placeholder && !suppress_flyouts && !egui::DragAndDrop::has_any_payload(ui.ctx()) {
                self.tool_rail_flyout(ui, &response, options);
            }
        }
        if self.tool_tooltip(response, tool).clicked() {
            self.activate_rail_tool(tool, ui.ctx());
        }
    }

    fn tool_rail_flyout(&mut self, ui: &egui::Ui, response: &egui::Response, options: &[Tool]) {
        let popup = egui::Popup::menu(response)
            .open_memory(
                (response.hovered() || response.clicked())
                    .then_some(egui::SetOpenCommand::Bool(true)),
            )
            .align(egui::RectAlign::TOP_START)
            .gap(0.)
            .width(190.)
            .show(|ui| {
                for &option in options {
                    let item = self.tool_menu_item(ui, option, self.tool == option);
                    if item.clicked() {
                        self.activate_rail_tool(option, ui.ctx());
                        ui.close();
                    }
                }
            });
        if let Some(popup) = popup {
            let hovering = ui
                .input(|input| input.pointer.hover_pos())
                .is_some_and(|pos| {
                    response.interact_rect.contains(pos) || popup.response.rect.contains(pos)
                });
            if !hovering {
                egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(response));
                ui.ctx().request_repaint();
            }
        }
    }

    fn active_rail_slot(&self) -> Tool {
        if self.tool.is_solid() {
            Tool::Union
        } else if Tool::DRAWING.contains(&self.tool) {
            Tool::Line
        } else if Tool::MEASUREMENT_MENU.contains(&self.tool) {
            Tool::Dimensions
        } else {
            self.tool
        }
    }

    fn tool_for_rail_slot(&self, slot: Tool) -> Tool {
        match slot {
            Tool::Line => self.drawing_tool,
            Tool::Dimensions => self.measurement_tool,
            Tool::Union => self.solid_tool,
            tool => tool,
        }
    }

    fn activate_rail_tool(&mut self, tool: Tool, ctx: &egui::Context) {
        if tool.is_solid() {
            self.start_solid_tool(tool, ctx);
        } else {
            self.set_tool(tool);
        }
    }

    fn cycle_rail_tools(&mut self, ui: &egui::Ui) {
        // Use raw wheel events for one tool change per input on a fully visible bar.
        let wheel = ui.input_mut(|input| {
            let mut delta = 0.;
            input.events.retain(|event| {
                if let egui::Event::MouseWheel { delta: scroll, .. } = event {
                    delta += scroll.y;
                    false
                } else {
                    true
                }
            });
            input.smooth_scroll_delta = Vec2::ZERO;
            delta
        });
        if wheel == 0. {
            return;
        }
        let active = self.active_rail_slot();
        let current = self
            .tool_rail_order
            .iter()
            .position(|&slot| slot == active)
            .unwrap_or(0);
        let next = if wheel > 0. {
            current.saturating_sub(1)
        } else {
            (current + 1).min(self.tool_rail_order.len() - 1)
        };
        if next != current {
            self.set_tool(self.tool_for_rail_slot(self.tool_rail_order[next]));
        }
    }

    pub(super) fn reorder_rail_tool(&mut self, tool: Tool, insertion: usize) {
        reorder_tool(&mut self.tool_rail_order, tool, insertion);
    }

    pub(super) fn reorder_rail_separator(&mut self, from: usize, insertion: usize) {
        move_separator(
            &mut self.tool_rail_separators,
            from,
            insertion.min(self.tool_rail_order.len()),
        );
    }
}

fn slide(ui: &egui::Ui, rect: Rect, content_left: f32, id: impl egui::AsIdSalt) -> Rect {
    // Animate relative to the content so scrolling does not lag.
    let offset = ui.ctx().animate_value_with_time(
        ui.scope_id().with(id),
        rect.left() - content_left,
        ui.style().animation_time.min(0.14),
    );
    rect.translate(egui::vec2(content_left + offset - rect.left(), 0.))
}

fn reorder_tool(order: &mut Vec<Tool>, tool: Tool, insertion: usize) {
    if let Some(from) = order.iter().position(|&slot| slot == tool) {
        let insertion = insertion.min(order.len());
        order.remove(from);
        order.insert(insertion - usize::from(from < insertion), tool);
    }
}

fn move_separator(separators: &mut [usize], from: usize, insertion: usize) -> bool {
    if separators.contains(&insertion) {
        return false;
    }
    if let Some(slot) = separators.iter_mut().find(|slot| **slot == from) {
        *slot = insertion;
        separators.sort_unstable();
        return true;
    }
    false
}
