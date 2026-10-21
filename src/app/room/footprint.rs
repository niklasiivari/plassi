//! The space each table takes on the seating chart: its top and a name card
//! beside every seat. Tables whose space overlaps cover each other's cards
//! there, so the room designer marks them.

use std::collections::{BTreeMap, BTreeSet};

use egui::{CornerRadius, Rect, Stroke, StrokeKind};

use super::*;
use crate::app::camera::Viewport;
use crate::app::turned::{TableTop, Turned, paint_turned, seat_card};
use crate::model::Layout;

pub(super) struct Footprint {
    top: TableTop,
    cards: Vec<Turned>,
    bounds: Rect,
}

impl Footprint {
    fn overlaps(&self, other: &Footprint) -> bool {
        if !self.bounds.intersects(other.bounds) {
            return false;
        }
        let shapes = |f: &Footprint| {
            let mut all = f.cards.clone();
            all.push(f.top.top);
            all
        };
        let theirs = shapes(other);
        shapes(self)
            .iter()
            .any(|a| theirs.iter().any(|b| a.overlaps(b)))
    }

    /// Faint outlines of the cards the chart draws around the table.
    pub(super) fn paint(&self, painter: &egui::Painter, view: &Viewport, color: egui::Color32) {
        let radius = CornerRadius::same((6.0 * view.zoom).clamp(1.0, 6.0) as u8);
        for card in &self.cards {
            let centre = view.point(card.centre);
            let rect = Rect::from_center_size(centre, card.size * view.zoom);
            paint_turned(painter, card.angle, centre, |painter| {
                painter.rect(
                    rect.shrink(1.0),
                    radius,
                    color.gamma_multiply(0.08),
                    Stroke::new(1.0, color.gamma_multiply(0.45)),
                    StrokeKind::Inside,
                )
            });
        }
    }
}

/// Each table's space on the chart, by table ID.
pub(super) fn footprints(
    layout: &Layout,
    tops: &[(&TableSpec, TableTop)],
) -> BTreeMap<String, Footprint> {
    let mut cards: BTreeMap<String, Vec<Turned>> = BTreeMap::new();
    for seat in layout.seats() {
        cards
            .entry(seat.table_id.clone())
            .or_default()
            .push(seat_card(&seat));
    }
    tops.iter()
        .map(|(table, top)| {
            let cards = cards.remove(&table.id).unwrap_or_default();
            let bounds = cards
                .iter()
                .map(Turned::bounds)
                .fold(top.bounds(), Rect::union);
            let print = Footprint {
                top: *top,
                cards,
                bounds,
            };
            (table.id.clone(), print)
        })
        .collect()
}

/// Tables whose space on the chart overlaps another table's.
pub(super) fn crowded(prints: &BTreeMap<String, Footprint>) -> BTreeSet<String> {
    let all: Vec<_> = prints.iter().collect();
    let mut crowded = BTreeSet::new();
    for (i, (a, first)) in all.iter().enumerate() {
        for (b, second) in &all[i + 1..] {
            if first.overlaps(second) {
                crowded.insert((*a).clone());
                crowded.insert((*b).clone());
            }
        }
    }
    crowded
}
