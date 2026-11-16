//! Settings → Agent connection, the status-bar control indicator and the
//! dialog that asks the designer to grant or deny access.
use super::*;
use crate::inspector_table::inspector_field;

/// The token with all but its last characters hidden.
pub(crate) fn masked(token: &str) -> String {
    let tail: String = token.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("••••••••-••••-••••-••••-••••••••{tail}")
}

/// Which copy button was pressed last, to confirm it next to that row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CopiedText {
    Token,
    Setup,
}

/// How long "Copied" stays next to the button.
const COPIED_SECONDS: f64 = 2.;

impl AgentLink {
    /// Puts `text` on the system clipboard and confirms it briefly.
    pub(crate) fn copy(&mut self, ctx: &egui::Context, text: String, what: CopiedText) {
        ctx.copy_text(text);
        self.copied = Some((what, ctx.input(|i| i.time) + COPIED_SECONDS));
        ctx.request_repaint_after(Duration::from_secs_f64(COPIED_SECONDS));
    }

    fn copied_label(&self, ui: &mut egui::Ui, what: CopiedText) {
        let now = ui.ctx().input(|i| i.time);
        if self.copied.is_some_and(|(copied, until)| copied == what && now < until) {
            ui.text_label(RichText::new(tr!("agent_link.copied")).color(Palette::for_ui(ui).muted));
        }
    }
}

impl BenchUpApp {
    pub(crate) fn agent_link_settings(&mut self, ui: &mut egui::Ui) {
        let colors = Palette::for_ui(ui);
        ui.text_weak(tr!("agent_link.help"));
        let prefs = &mut self.general_settings.agent;
        ui.checkbox(&mut prefs.enabled, tr!("agent_link.allow")).on_hover_text(tr!("agent_link.allow_help"));
        inspector_field(ui, "settings/agent_connection/port", tr!("agent_link.port"), |ui| {
            ui.add(egui::DragValue::new(&mut prefs.port).range(1024..=65535).speed(1))
                .on_hover_text(tr!("agent_link.port_help"))
        });
        prefs.normalize();
        let port = prefs.port;
        let status = match &self.agent_link.status {
            Status::Off => tr!("agent_link.status.off").to_owned(),
            Status::Connecting => tr!("agent_link.status.connecting").to_owned(),
            Status::Connected { client: None } => tr!("agent_link.status.waiting").to_owned(),
            Status::Connected { client: Some(client) } => tr!("agent_link.status.connected", client = client),
            Status::Error(message) => tr!("agent_link.status.error", message = message),
        };
        let status_color = if self.agent_link.connected() { colors.ink } else { colors.muted };
        ui.text_label(RichText::new(status).color(status_color));

        ui.separator();
        ui.text_strong(tr!("agent_link.token"));
        self.agent_link.load_token();
        match self.agent_link.token.clone() {
            None => {
                ui.text_weak(tr!("agent_link.token_none"));
            }
            Some(token) => {
                let shown = if self.agent_link.show_token { token } else { masked(&token) };
                ui.text_label(RichText::new(shown).monospace());
            }
        }
        if let Some(error) = &self.agent_link.token_error {
            ui.text_label(RichText::new(tr!("agent_link.token_save_failed", error = error)).color(colors.accent));
        }
        ui.text_weak(tr!("agent_link.token_help"));
        let has_token = self.agent_link.token.is_some();
        ui.horizontal_wrapped(|ui| {
            let label = if self.agent_link.show_token { tr!("agent_link.hide") } else { tr!("agent_link.show") };
            if ui.add_enabled(has_token, egui::Button::new(label)).clicked() {
                self.agent_link.show_token = !self.agent_link.show_token;
            }
            if ui.add_enabled(has_token, egui::Button::new(tr!("agent_link.copy_token"))).clicked()
                && let Some(token) = self.agent_link.token.clone()
            {
                self.agent_link.copy(ui.ctx(), token, CopiedText::Token);
            }
            let label = if has_token { tr!("agent_link.regenerate") } else { tr!("agent_link.generate") };
            if ui.button(label).on_hover_text(tr!("agent_link.regenerate_help")).clicked() {
                self.agent_link.regenerate();
            }
            self.agent_link.copied_label(ui, CopiedText::Token);
        });

        ui.separator();
        ui.text_strong(tr!("agent_link.setup"));
        ui.text_weak(tr!("agent_link.setup_help"));
        if let Some(token) = self.agent_link.token.clone() {
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr!("agent_link.copy_command")).on_hover_text(tr!("agent_link.command_help")).clicked() {
                    self.agent_link.copy(ui.ctx(), claude_command(&token, port), CopiedText::Setup);
                }
                if ui.button(tr!("agent_link.copy_codex_command")).on_hover_text(tr!("agent_link.codex_command_help")).clicked() {
                    self.agent_link.copy(ui.ctx(), codex_command(&token, port), CopiedText::Setup);
                }
                if ui.button(tr!("agent_link.copy_json")).on_hover_text(tr!("agent_link.json_help")).clicked() {
                    self.agent_link.copy(ui.ctx(), mcp_json(&token, port), CopiedText::Setup);
                }
                if ui.button(tr!("agent_link.copy_shell")).on_hover_text(tr!("agent_link.shell_help")).clicked() {
                    self.agent_link.copy(ui.ctx(), shell_setup(&token, port), CopiedText::Setup);
                }
                self.agent_link.copied_label(ui, CopiedText::Setup);
            });
        }
    }

    /// The connected agent's activity and controls for taking or releasing control.
    pub(crate) fn agent_status_chip(&mut self, ui: &mut egui::Ui) {
        if !self.general_settings.agent.enabled || !self.agent_link.connected() {
            return;
        }
        let colors = Palette::for_ui(ui);
        let now = super::clock();
        let client = self.agent_link.client_name().map(str::to_owned);
        // Laid out right to left: the button, then the text, then the dot.
        let (text, color) = if self.agent_link.control == Control::Designer {
            if ui.small_button(tr!("agent_link.release_control")).on_hover_text(tr!("agent_link.release_control_help")).clicked() {
                self.agent_link.release_control();
            }
            (tr!("agent_link.chip.designer").to_owned(), colors.muted)
        } else if self.agent_link.editing(now) {
            if ui.small_button(tr!("agent_link.take_control")).on_hover_text(tr!("agent_link.take_control_help")).clicked() {
                self.agent_link.take_control();
            }
            (tr!("agent_link.chip.editing").to_owned(), colors.accent)
        } else {
            (tr!("agent_link.chip.connected").to_owned(), colors.muted)
        };
        let text = self.agent_link.activity_text(now).unwrap_or(text);
        let tooltip = match client {
            Some(client) => format!("{text}\n{client}"),
            None => text.clone(),
        };
        let label = ui.add(crate::natural_text::Label::new(RichText::new(text).size(12.).color(color)).truncate());
        label.on_hover_text(tooltip);
        // The dot reports the live connection; text and buttons report control.
        dot(ui, Color32::from_rgb(34, 197, 94));
        ui.add_space(8.);
    }

    /// Asks the designer to grant or deny access while the agent waits.
    pub(crate) fn agent_access_window(&mut self, ctx: &egui::Context) {
        if !self.agent_link.asking {
            return;
        }
        let client = self.agent_link.waiting_client().filter(|c| !c.is_empty()).unwrap_or(tr!("agent_link.an_agent")).to_owned();
        let waiting = self.agent_link.waiting_changes();
        let designer = self.agent_link.control == Control::Designer;
        let (mut grant, mut deny) = (false, false);
        Modal::new("agent_access", 440.).dismissible(false).show(ctx, |modal| {
            modal.header(|ui| {
                ui.text_heading(tr!("agent_link.permission.title"));
            });
            modal.body(|ui| {
                ui.text_label(tr!("agent_link.permission.body", client = client));
                ui.text_label(if designer {
                    tr!("agent_link.permission.designer")
                } else {
                    tr!("agent_link.permission.busy")
                });
                ui.text_weak(tr!("agent_link.permission.waiting", count = waiting));
            });
            modal.footer(|ui| {
                // Least impactful first: it sits at the right edge.
                if ui.button(tr!("agent_link.deny")).clicked() {
                    deny = true;
                }
                if primary_button(ui, tr!("agent_link.grant"), true).clicked() {
                    grant = true;
                }
            });
        });
        if grant {
            self.grant_agent_access(ctx);
        } else if deny {
            self.agent_link.deny();
            ctx.request_repaint();
        }
    }
}
