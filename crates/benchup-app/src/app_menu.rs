use super::{BenchUpApp, egui};
use egui::menu::{MenuConfig, MenuState};
use egui::{Id, Key, PointerButton, Popup, PopupCloseBehavior, Response, Sense, Ui};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuPress {
    Held,
    Released,
    ReleasedOnTitle,
}

impl MenuPress {
    fn id() -> Id {
        Id::new("app_bar_menu_press_in_popup")
    }

    fn in_popup(ctx: &egui::Context) -> Option<Self> {
        ctx.data(|data| data.get_temp(Self::id()))
    }

    pub(super) fn submenu_config(ui: &Ui) -> MenuConfig {
        let behavior = if Self::in_popup(ui.ctx()) == Some(Self::ReleasedOnTitle) {
            PopupCloseBehavior::IgnoreClicks
        } else {
            PopupCloseBehavior::CloseOnClickOutside
        };
        MenuConfig::new().close_behavior(behavior)
    }
}

pub(super) trait MenuItemResponse {
    fn menu_press_target(&self) -> Option<MenuPress>;

    fn menu_activated(&self) -> bool;
}

impl MenuItemResponse for Response {
    fn menu_press_target(&self) -> Option<MenuPress> {
        let press = MenuPress::in_popup(&self.ctx)?;
        let targeted = press != MenuPress::ReleasedOnTitle
            && self.visible()
            && self.enabled()
            && self.container_contains_pointer();
        if targeted && press == MenuPress::Held {
            self.ctx.highlight_widget(self.id);
        }
        if self.highlighted() != (targeted && press == MenuPress::Held) {
            self.ctx.request_repaint();
        }
        targeted.then_some(press)
    }

    fn menu_activated(&self) -> bool {
        self.clicked() || self.menu_press_target() == Some(MenuPress::Released)
    }
}

pub(super) struct AppBarMenus {
    active_id: Id,
    held_id: Id,
    press: Option<MenuPress>,
    primary_pressed: bool,
    released_on_title: bool,
}

impl AppBarMenus {
    pub(super) fn new(ui: &Ui) -> Self {
        let held_id = ui.scope_id().with("main_menu_primary_held");
        let was_held = ui.data(|data| data.get_temp::<bool>(held_id).unwrap_or(false));
        let primary = PointerButton::Primary;
        let (primary_pressed, press) = ui.input(|input| {
            if !input.focused || input.key_pressed(Key::Escape) {
                return (false, None);
            }
            let press = if was_held && input.pointer.button_released(primary) {
                Some(MenuPress::Released)
            } else if was_held && input.pointer.button_down(primary) {
                Some(MenuPress::Held)
            } else {
                None
            };
            (input.pointer.button_pressed(primary), press)
        });
        Self {
            active_id: ui.scope_id().with("main_menu_active_popup"),
            held_id,
            press,
            primary_pressed,
            released_on_title: false,
        }
    }

    pub(super) fn show(&mut self, ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
        // Own the drag too, so dragging off a title cannot move the desktop window.
        let response = ui.add(
            egui::Button::new(title)
                .stroke(egui::Stroke::NONE)
                .sense(Sense::click_and_drag()),
        );
        // A pointer click is only needed if the press and release arrived together.
        let clicked = response.clicked()
            && (self.primary_pressed || !response.clicked_by(PointerButton::Primary));
        let pressed = self.primary_pressed && response.is_pointer_button_down_on();
        if pressed {
            self.press = Some(MenuPress::Held);
        }
        let popup_id = Popup::default_response_id(&response);
        let over_title = response.container_contains_pointer();
        let released_here = self.press == Some(MenuPress::Released) && over_title;
        self.released_on_title |= released_here;

        let active = ui.data(|data| data.get_temp::<Id>(self.active_id));
        let can_switch = active.is_some_and(|id| id != popup_id && Popup::is_id_open(ui.ctx(), id));
        let mut open = pressed || clicked || (can_switch && self.press.is_some() && over_title);
        let hover_id = response.id.with("main_menu_hover_started");
        if can_switch && self.press.is_none() && response.hovered() && !clicked {
            let now = ui.input(|input| input.time);
            let started = ui.data_mut(|data| *data.get_temp_mut_or_insert_with(hover_id, || now));
            let remaining = 0.5 - (now - started);
            if remaining <= 0.0 {
                open = true;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(remaining));
            }
        } else {
            ui.data_mut(|data| data.remove::<f64>(hover_id));
        }

        let mut config = MenuConfig::new().style(BenchUpApp::menu_style);
        if clicked || released_here {
            // A title sits outside its popup and any nested submenus.
            config.close_behavior = PopupCloseBehavior::IgnoreClicks;
        }
        let inner = Popup::menu(&response)
            .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)))
            .close_behavior(config.close_behavior)
            .style(config.style.clone())
            .info(
                egui::UiStackInfo::new(egui::UiKind::Menu)
                    .with_tag_value(MenuConfig::MENU_CONFIG_TAG, config),
            )
            .show(|ui| {
                if let Some(press) = self.press {
                    let press = if released_here {
                        MenuPress::ReleasedOnTitle
                    } else {
                        press
                    };
                    ui.data_mut(|data| data.insert_temp(MenuPress::id(), press));
                }
                content(ui);
                ui.data_mut(|data| data.remove::<MenuPress>(MenuPress::id()));
            });
        if inner.is_some() {
            ui.data_mut(|data| data.insert_temp(self.active_id, popup_id));
        }
    }

    pub(super) fn finish(self, ui: &Ui) {
        ui.data_mut(|data| data.insert_temp(self.held_id, self.press == Some(MenuPress::Held)));

        // After a drag, egui may report a release without a click to dismiss the popup.
        if self.press != Some(MenuPress::Released) || self.released_on_title {
            return;
        }
        let Some(root_id) = ui.data(|data| data.get_temp::<Id>(self.active_id)) else {
            return;
        };
        let release_layer = ui
            .input(|input| input.pointer.interact_pos())
            .and_then(|pos| ui.ctx().layer_id_at(pos));
        let mut menu_id = root_id;
        loop {
            if release_layer == Some(egui::LayerId::new(egui::Order::Foreground, menu_id)) {
                return;
            }
            let Some(submenu_id) = MenuState::from_id(ui.ctx(), menu_id, |state| state.open_item)
            else {
                break;
            };
            menu_id = submenu_id;
        }
        Popup::close_id(ui.ctx(), root_id);
    }
}
