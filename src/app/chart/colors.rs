//! Colouring the chart by a category: organization, ticket type, food or drink.

use super::*;

/// One category in view: its key, name and how many people belong to it.
pub(super) struct ColorEntry {
    pub(super) key: String,
    pub(super) name: String,
    pub(super) count: usize,
}

/// The categories of one kind, most common first; the first few get a colour
/// of their own and the rest share "Other".
pub(super) struct Coloring {
    pub(super) kind: Kind,
    pub(super) entries: Vec<ColorEntry>,
    /// People with nothing written for this kind.
    pub(super) blank: usize,
}

impl Coloring {
    pub(super) fn new(project: &Project, kind: Kind) -> Self {
        let mut entries: Vec<ColorEntry> = Vec::new();
        let mut blank = 0;
        for attendee in &project.attendees {
            let key = project.category_key(kind, attendee);
            if key.is_empty() {
                blank += 1;
                continue;
            }
            match entries.iter_mut().find(|e| e.key == key) {
                Some(entry) => entry.count += 1,
                None => entries.push(ColorEntry {
                    key,
                    name: project.category_name(kind, attendee),
                    count: 1,
                }),
            }
        }
        entries.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        Self {
            kind,
            entries,
            blank,
        }
    }

    /// The colour of a category key; muted for "Other" and `None` for blank.
    pub(super) fn color(&self, key: &str, p: Palette) -> Option<Color32> {
        if key.is_empty() {
            return None;
        }
        let index = self.entries.iter().position(|e| e.key == key)?;
        Some(theme::category_color(index, is_dark(p)).unwrap_or(p.muted))
    }

    /// Entries that have a colour of their own, and how many people are in the rest.
    pub(super) fn legend(&self, p: Palette) -> (Vec<(&ColorEntry, Color32)>, usize) {
        let mut own = Vec::new();
        let mut other = 0;
        for (index, entry) in self.entries.iter().enumerate() {
            match theme::category_color(index, is_dark(p)) {
                Some(color) => own.push((entry, color)),
                None => other += entry.count,
            }
        }
        (own, other)
    }
}

impl PlassiApp {
    /// The "Colour by" choice and, when a category is chosen, its legend.
    pub(super) fn color_section(&mut self, ui: &mut egui::Ui, p: Palette) {
        theme::section_title(ui, p, tr("Colour cards by"));
        let name = |kind: Option<Kind>| kind.map_or(tr("Nothing"), Kind::plural);
        let mut chosen = self.chart.color_by;
        theme::combo("color-by", name(chosen))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                let kinds = std::iter::once(None).chain(Kind::ALL.map(Some));
                for kind in kinds {
                    ui.selectable_value(&mut chosen, kind, name(kind));
                }
            });
        self.chart.color_by = chosen;
        if let Some(coloring) = self.coloring() {
            ui.add_space(theme::space::S);
            self.color_legend(ui, p, coloring);
        }
    }

    /// The legend for the chosen colouring, with counts.
    fn color_legend(&self, ui: &mut egui::Ui, p: Palette, coloring: &Coloring) {
        let (own, other) = coloring.legend(p);
        theme::text_rows(ui, |ui| {
            for (entry, color) in own {
                legend_row(ui, p, color, &entry.name, entry.count);
            }
            if other > 0 {
                legend_row(ui, p, p.muted, tr("Other"), other);
            }
            if coloring.blank > 0 {
                legend_row(ui, p, p.border, tr("Not given"), coloring.blank);
            }
        });
    }
}

fn legend_row(ui: &mut egui::Ui, p: Palette, color: Color32, name: &str, count: usize) {
    ui.horizontal(|ui| {
        theme::status_dot(ui, color);
        ui.label(RichText::new(name).size(theme::text::SMALL));
        theme::hint(ui, p, count.to_string());
    });
}

fn is_dark(p: Palette) -> bool {
    p.bg.r() < 100
}
