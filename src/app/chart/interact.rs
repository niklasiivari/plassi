//! Seat cards and loose cards on the canvas: clicks, drags and the
//! right-click menu, collected per frame and applied afterwards.

use super::drag::Targets;
use super::*;
use crate::app::people::RosterEdit;

mod menu;

/// What the user did with cards this frame.
#[derive(Default)]
pub(super) struct CardActions {
    /// `Some(None)` clicks an empty seat, which clears the selection.
    clicked: Option<Option<usize>>,
    /// The person grabbed and where on their card.
    drag_start: Option<(usize, Vec2)>,
    hovered_card: bool,
    hovered_person: Option<usize>,
    /// Ctrl/Shift-click: add to or remove from the multi-selection.
    pick: Option<usize>,
    /// Pin a person, or unpin them (`None`).
    pin: Option<(usize, Option<Pin>)>,
    /// Pin the selected people to their seats, or unpin them all.
    pin_all: Option<Vec<usize>>,
    /// Keep these people at this table.
    keep: Option<(Vec<usize>, String)>,
    start_swap: Option<usize>,
    /// Moves picked by clicking in swap mode or from the menu.
    moves: Option<Targets>,
    /// Open a roster change for a person on the People page.
    roster: Option<(usize, RosterEdit)>,
    remove: Option<usize>,
}

/// Where a card is: on a seat (taken or not), or loose with a person.
#[derive(Clone, Copy)]
enum Place<'a> {
    Seat(&'a Seat),
    Loose(usize, Pos2),
}

impl Place<'_> {
    fn person(&self, scene: &Scene) -> Option<usize> {
        match self {
            Place::Seat(seat) => scene.occupant.get(&seat.id).copied(),
            Place::Loose(person, _) => Some(*person),
        }
    }

    fn world_rect(&self) -> Rect {
        match self {
            Place::Seat(seat) => card_rect(seat),
            Place::Loose(_, at) => Rect::from_center_size(*at, Vec2::new(CARD_W, CARD_H)),
        }
    }
}

impl PlassiApp {
    /// Paints every seat card, then the loose cards on top, and collects
    /// what the user did with them.
    pub(super) fn cards(
        &self,
        ui: &mut egui::Ui,
        paint: &Paint,
        targets: Option<&Targets>,
    ) -> CardActions {
        let landing: HashSet<&str> = targets
            .into_iter()
            .flatten()
            .filter_map(|(_, spot)| match spot {
                Spot::Seat(id) => Some(id.as_str()),
                _ => None,
            })
            .collect();
        let mut actions = CardActions::default();
        for seat in &paint.scene.seats {
            let drop = landing.contains(seat.id.as_str());
            self.card(ui, paint, Place::Seat(seat), drop, &mut actions);
        }
        for (&person, &at) in &paint.scene.loose {
            self.card(ui, paint, Place::Loose(person, at), false, &mut actions);
        }
        actions
    }

    /// One card: paints it and records clicks, drags and menu choices.
    fn card(
        &self,
        ui: &mut egui::Ui,
        paint: &Paint,
        place: Place,
        drop: bool,
        actions: &mut CardActions,
    ) {
        let Paint {
            painter,
            view,
            p,
            scene,
        } = *paint;
        let screen = view.rect(place.world_rect());
        if !screen.intersects(view.clip) {
            return;
        }
        let person = place.person(scene);
        let id = match place {
            Place::Seat(seat) => ui.id().with(("seat", &seat.id)),
            Place::Loose(person, _) => ui.id().with(("loose", person)),
        };
        let response = ui
            .interact(screen, id, Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab);
        actions.hovered_card |= response.hovered();
        if response.hovered() {
            actions.hovered_person = person;
        }
        if let Some(attendee) = person.and_then(|id| self.project.person(id)) {
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &attendee.name)
            });
        }
        let look = self.card_look(scene, place, person, response.hovered() || drop);
        self.paint_card(painter, p, screen, person, look);
        self.card_clicks(ui, paint, &response, place, actions);
        let Some(person) = person else { return };
        egui::Popup::context_menu(&response)
            .style(theme::menu_style)
            .show(|ui| self.card_menu(ui, scene, place, person, actions));
        if self.chart.drag.is_none() {
            let seat = match place {
                Place::Seat(seat) => Some(seat.id.as_str()),
                Place::Loose(..) => None,
            };
            response.on_hover_ui(|ui| self.hover_card(ui, p, scene, person, seat));
        }
    }

    fn card_look(
        &self,
        scene: &Scene,
        place: Place,
        person: Option<usize>,
        hovered: bool,
    ) -> CardLook {
        let needle = crate::app::search::Needle::new(&self.chart.search);
        let search_miss = !needle.is_empty()
            && !person
                .and_then(|id| self.project.person(id))
                .is_some_and(|a| needle.matches(&a.name));
        let dragged = person.is_some_and(|id| {
            self.chart
                .drag
                .as_ref()
                .is_some_and(|d| d.people.contains(&id))
        });
        CardLook {
            status: person.and_then(|id| scene.status(id)),
            selected: person.is_some() && person == self.selected_person,
            hovered,
            dimmed: search_miss || dragged,
            pin: person.and_then(|id| scene.pin_mark(id)),
            moved: person.is_some_and(|id| scene.moved.contains(&id)),
            gap: matches!(place, Place::Seat(seat) if scene.gaps.contains(&seat.id)),
            loose: matches!(place, Place::Loose(..)),
            placing: self.placing(scene, place).is_some(),
        }
    }

    /// Click, Ctrl-click, click in swap mode, and the start of a drag.
    fn card_clicks(
        &self,
        ui: &egui::Ui,
        paint: &Paint,
        response: &egui::Response,
        place: Place,
        actions: &mut CardActions,
    ) {
        let person = place.person(paint.scene);
        let adding = ui.input(|i| i.modifiers.command || i.modifiers.shift);
        if response.clicked() && adding {
            actions.pick = person;
        } else if response.clicked() {
            match (self.chart.swap_from, self.placing(paint.scene, place)) {
                (Some(from), _) => actions.moves = Some(swap_moves(paint.scene, from, place)),
                (None, Some(moves)) => actions.moves = Some(moves),
                (None, None) => actions.clicked = Some(person),
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(person) = person
        {
            let pressed = ui
                .input(|i| i.pointer.press_origin())
                .unwrap_or(response.rect.center());
            let grip = paint.view.world(pressed) - place.world_rect().center();
            actions.drag_start = Some((person, grip));
        }
    }

    /// Clicking an empty seat while someone without a seat is selected seats
    /// them there.
    fn placing(&self, scene: &Scene, place: Place) -> Option<Targets> {
        let Place::Seat(seat) = place else {
            return None;
        };
        if self.chart.drag.is_some() {
            return None;
        }
        let person = self.selected_person?;
        let free = !scene.occupant.contains_key(&seat.id);
        (free && scene.unseated.contains(&person))
            .then(|| vec![(person, Spot::Seat(seat.id.clone()))])
    }

    pub(super) fn apply_card_actions(
        &mut self,
        ui: &egui::Ui,
        background: &egui::Response,
        actions: CardActions,
    ) {
        if let Some(person) = actions.remove {
            self.remove_people(&[person]);
        }
        if let Some((person, edit)) = actions.roster {
            self.selected_person = Some(person);
            self.people.roster = Some(edit);
            self.page = crate::app::Page::People;
        }
        if let Some(person) = actions.pick {
            self.toggle_pick(person);
        }
        if actions.hovered_person != self.chart.hovered_person {
            self.chart.hovered_person = actions.hovered_person;
            ui.ctx().request_repaint();
        }
        if let Some(person) = actions.start_swap {
            self.chart.swap_from = Some(person);
            self.selected_person = Some(person);
        }
        if let Some(moves) = actions.moves {
            self.chart.swap_from = None;
            self.rearrange(&moves);
        }
        self.escape_pressed(ui);
        if let Some((person, pin)) = actions.pin {
            self.set_pin(person, pin);
        }
        if let Some(people) = actions.pin_all {
            self.toggle_seat_pins(&people);
        }
        if let Some((people, table)) = actions.keep {
            self.keep_at_table(&people, &table);
        }
        if let Some((person, grip)) = actions.drag_start {
            self.start_drag(person, grip);
        }
        if let Some(person) = actions.clicked {
            self.selected_person = person;
        } else if background.clicked() && !actions.hovered_card {
            self.selected_person = None;
        }
    }

    /// Adds a person to the multi-selection or takes them out. The first
    /// pick also takes in whoever was selected.
    pub(super) fn toggle_pick(&mut self, person: usize) {
        let seed = self
            .chart
            .multi
            .is_empty()
            .then_some(self.selected_person)
            .flatten()
            .filter(|id| self.project.person(*id).is_some());
        if let Some(selected) = seed {
            self.chart.multi.insert(selected);
        }
        if Some(person) != seed && !self.chart.multi.remove(&person) {
            self.chart.multi.insert(person);
        }
    }

    /// Esc cancels, one step at a time: swap mode, the selection, a drag or
    /// box, then the selected person.
    fn escape_pressed(&mut self, ui: &egui::Ui) {
        let typing = ui.ctx().memory(|m| m.focused().is_some());
        if !typing
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
            && self.chart.swap_from.take().is_none()
            && std::mem::take(&mut self.chart.multi).is_empty()
            && self.chart.drag.take().is_none()
            && self.chart.marquee.take().is_none()
        {
            self.selected_person = None;
        }
    }
}

/// The moves that swap `from` with the card clicked in swap mode. A loose
/// person takes `from`'s place; a seat takes `from`, and its occupant, if
/// any, takes `from`'s place.
fn swap_moves(scene: &Scene, from: usize, place: Place) -> Targets {
    match place {
        Place::Seat(seat) => vec![(from, Spot::Seat(seat.id.clone()))],
        Place::Loose(other, at) => vec![
            (from, Spot::Loose(crate::model::Point { x: at.x, y: at.y })),
            (other, scene.spot_of(from)),
        ],
    }
}
