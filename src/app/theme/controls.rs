//! egui's built-in controls (dropdowns, sliders) restyled for the Soft
//! treatment. They take their look from widget visuals, so each helper
//! scopes the visuals around the one control.

use std::fmt::Debug;
use std::hash::Hash;

use eframe::egui::{self, Stroke};

use super::icons::{Icon, paint};

/// Field look: the text-field fill and a hairline outline that darkens a
/// little on hover. Shared by text fields and dropdowns.
pub(super) fn field_visuals(ui: &mut egui::Ui) {
    let visuals = ui.visuals_mut();
    let line = visuals.widgets.noninteractive.bg_stroke;
    let fill = visuals.text_edit_bg_color();
    let w = &mut visuals.widgets;
    let hover = Stroke::new(
        1.0,
        super::blend(line.color, w.hovered.fg_stroke.color, 0.25),
    );
    w.inactive.bg_stroke = line;
    w.hovered.bg_stroke = hover;
    for state in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        state.weak_bg_fill = fill;
    }
}

/// A dropdown that looks like a [`super::field`]: same height, corners and
/// outline, with the app's chevron.
pub struct Combo(egui::ComboBox, egui::Id, Option<f32>);

#[derive(Clone, Default)]
struct ComboSearch {
    query: String,
    open: bool,
    active: usize,
    count: usize,
    focus_visible: bool,
}

pub fn combo(id_salt: impl Hash + Debug, selected: impl Into<egui::WidgetText>) -> Combo {
    let id = egui::Id::new(&id_salt);
    Combo(
        egui::ComboBox::from_id_salt(id_salt)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .selected_text(selected)
            .truncate()
            .icon(|ui, rect, visuals, _open| {
                let color = super::blend(visuals.text_color(), ui.visuals().panel_fill, 0.35);
                paint(ui.painter(), Icon::ChevronDown, rect.center(), color);
            }),
        id,
        None,
    )
}

impl Combo {
    pub fn width(self, width: f32) -> Self {
        Self(self.0, self.1, Some(width))
    }

    pub fn close_behavior(self, behavior: egui::PopupCloseBehavior) -> Self {
        Self(self.0.close_behavior(behavior), self.1, self.2)
    }

    pub fn show_ui<R>(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut ComboOptions<'_>) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let id = ui.make_persistent_id(self.1).with("search-state");
        let mut search = ui
            .ctx()
            .data_mut(|data| data.get_temp::<ComboSearch>(id))
            .unwrap_or_default();
        let mut open = false;
        let result = ui
            .scope(|ui| {
                field_visuals(ui);
                // egui grows the frame to the control height at the bottom only;
                // padding it evenly keeps the text centred like a field's.
                let text = ui.text_style_height(&egui::TextStyle::Button);
                let padding = egui::vec2(
                    super::space::M,
                    ((super::CONTROL_HEIGHT - text) / 2.0).max(0.0),
                );
                ui.spacing_mut().button_padding = padding;
                let width = self
                    .2
                    .unwrap_or(ui.spacing().combo_width)
                    .min(ui.available_width());
                // A compact trigger (e.g. the arrow beside a text field) must
                // not constrain the searchable menu to the trigger's width.
                let frame_margin = egui::Frame::popup(ui.style()).total_margin().sum().x;
                let available =
                    (ui.ctx().content_rect().width() - 2.0 * super::space::S - frame_margin)
                        .max(1.0);
                let menu_width = width.max(240.0).min(available);
                self.0.width(width).show_ui(ui, |ui| {
                    ui.set_width(menu_width);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    open = true;
                    let fresh = !search.open;
                    if fresh {
                        search.query.clear();
                        search.active = 0;
                        search.focus_visible = false;
                    }
                    let (up, down, enter) = ui.input_mut(|input| {
                        (
                            input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                        )
                    });
                    if up || down {
                        search.focus_visible = true;
                    }
                    if up {
                        search.active = search.active.saturating_sub(1);
                    }
                    if down {
                        search.active = (search.active + 1).min(search.count.saturating_sub(1));
                    }
                    let label = crate::i18n::tr("Search options…");
                    let response = ui.add(
                        super::field(&mut search.query)
                            .id(id.with("input"))
                            .hint_text(label)
                            .desired_width(ui.available_width().min(320.0)),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, label)
                    });
                    if fresh {
                        response.request_focus();
                    }
                    if response.changed() {
                        search.active = 0;
                        search.focus_visible = !search.query.trim().is_empty();
                    }
                    ui.separator();
                    let mut options = ComboOptions {
                        ui,
                        query: search.query.trim().to_lowercase(),
                        active: search.active,
                        enter,
                        moved: up || down,
                        count: 0,
                        focus_visible: search.focus_visible,
                    };
                    let result = add(&mut options);
                    search.count = options.count;
                    if options.count == 0 {
                        options.ui.label(crate::i18n::tr("No matching options"));
                    }
                    result
                })
            })
            .inner;
        search.open = open;
        ui.ctx().data_mut(|data| data.insert_temp(id, search));
        result
    }
}

/// All options pass through the same filtering and keyboard selection, including
/// multi-select checkboxes. Filtering never changes a stored selection.
pub struct ComboOptions<'a> {
    ui: &'a mut egui::Ui,
    query: String,
    active: usize,
    enter: bool,
    moved: bool,
    count: usize,
    focus_visible: bool,
}

impl ComboOptions<'_> {
    fn focus_outline(&self, response: &egui::Response, active: bool) {
        if active && self.focus_visible {
            self.ui.painter().rect_stroke(
                response.rect,
                super::radius::SM,
                Stroke::new(1.0, super::palette(self.ui.ctx()).accent),
                egui::StrokeKind::Inside,
            );
        }
    }

    fn matching(&mut self, label: &str) -> Option<bool> {
        let label = label.to_lowercase();
        if !self
            .query
            .split_whitespace()
            .all(|word| label.contains(word))
        {
            return None;
        }
        let active = self.count == self.active;
        self.count += 1;
        Some(active)
    }

    pub fn action(&mut self, selected: bool, label: &str) -> bool {
        let Some(active) = self.matching(label) else {
            return false;
        };
        let response = self.ui.selectable_label(selected, label);
        self.focus_outline(&response, active);
        if active && self.moved {
            response.scroll_to_me(None);
        }
        if response.clicked() || (active && self.enter) {
            self.ui.close();
            return true;
        }
        false
    }

    pub fn value<T: PartialEq>(&mut self, current: &mut T, value: T, label: &str) -> bool {
        if self.action(*current == value, label) {
            let changed = *current != value;
            *current = value;
            changed
        } else {
            false
        }
    }

    pub fn checkbox(&mut self, enabled: bool, selected: &mut bool, label: &str) -> bool {
        let Some(active) = self.matching(label) else {
            return false;
        };
        let response = self
            .ui
            .add_enabled(enabled, egui::Checkbox::new(selected, label));
        if active && self.moved {
            response.scroll_to_me(None);
        }
        if enabled {
            self.focus_outline(&response, active);
        }
        if enabled && active && self.enter {
            *selected = !*selected;
            return true;
        }
        response.changed()
    }
}
