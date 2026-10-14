//! Colouring the chart by a category: organization, ticket type, food or drink.

use super::*;

/// The app setting that remembers the colouring.
pub(in crate::app) const COLOR_SETTING: &str = "chart-colour";

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
                for kind in std::iter::once(None).chain(Kind::ALL.map(Some)) {
                    ui.selectable_value(&mut chosen, kind, name(kind));
                }
            });
        if chosen != self.chart.color_by {
            self.set_color_by(chosen);
        }
        let Some(kind) = chosen else { return };
        let project = &self.project;
        let coloring = self
            .chart
            .coloring
            .get((self.file.revision, kind), || Coloring::new(project, kind));
        ui.add_space(theme::space::S);
        self.chart.color_hover = color_legend(ui, p, coloring);
    }

    /// Colours cards by `kind` (or by nothing) and remembers the choice.
    pub(super) fn set_color_by(&mut self, kind: Option<Kind>) {
        self.chart.color_by = kind;
        let key = kind.map_or("", Kind::key);
        self.file.library.set_setting(COLOR_SETTING, key);
    }

    /// The next colouring in the dropdown's order, for the C key.
    pub(super) fn next_color_by(&mut self) {
        let order: Vec<Option<Kind>> = std::iter::once(None).chain(Kind::ALL.map(Some)).collect();
        let at = order
            .iter()
            .position(|k| *k == self.chart.color_by)
            .unwrap_or(0);
        self.set_color_by(order[(at + 1) % order.len()]);
    }

    /// Whether a person's card dims because another category is under the
    /// pointer in the legend.
    pub(super) fn color_dims(&self, person: usize) -> bool {
        let (Some(hovered), Some(coloring)) = (&self.chart.color_hover, self.coloring()) else {
            return false;
        };
        self.project
            .person(person)
            .is_some_and(|a| self.project.category_key(coloring.kind, a) != *hovered)
    }
}

/// The legend for a colouring, with counts. Returns the key of the category
/// under the pointer ("" for "Not given"; nothing for "Other").
fn color_legend(ui: &mut egui::Ui, p: Palette, coloring: &Coloring) -> Option<String> {
    let (own, other) = coloring.legend(p);
    let mut hovered = None;
    theme::text_rows(ui, |ui| {
        for (entry, color) in own {
            if legend_row(ui, p, color, &entry.name, entry.count) {
                hovered = Some(entry.key.clone());
            }
        }
        if other > 0 {
            legend_row(ui, p, p.muted, tr("Other"), other);
        }
        if coloring.blank > 0 && legend_row(ui, p, p.border, tr("Not given"), coloring.blank) {
            hovered = Some(String::new());
        }
    });
    hovered
}

/// One legend line; returns whether the pointer is over it.
fn legend_row(ui: &mut egui::Ui, p: Palette, color: Color32, name: &str, count: usize) -> bool {
    let row = ui.horizontal(|ui| {
        theme::status_dot(ui, color);
        ui.label(RichText::new(name).size(theme::text::SMALL));
        theme::hint(ui, p, count.to_string());
    });
    ui.rect_contains_pointer(row.response.rect)
}

fn is_dark(p: Palette) -> bool {
    p.bg.r() < 100
}

#[cfg(test)]
mod tests {
    use super::*;

    fn people_in(organizations: &[&str]) -> Project {
        let mut project = crate::tests::two_tables();
        project.attendees.clear();
        for (id, organization) in organizations.iter().enumerate() {
            let mut attendee = crate::example::project().attendees[0].clone();
            attendee.id = id;
            attendee.name = format!("Person {id}");
            attendee.organization = organization.to_string();
            project.attendees.push(attendee);
        }
        project
    }

    #[test]
    fn nobody_means_an_empty_legend() {
        let coloring = Coloring::new(&people_in(&[]), Kind::Organization);
        assert!(coloring.entries.is_empty() && coloring.blank == 0);
        let (own, other) = coloring.legend(theme::LIGHT);
        assert!(own.is_empty() && other == 0);
    }

    #[test]
    fn categories_past_the_palette_share_other() {
        let names: Vec<String> = (0..14).map(|i| format!("Org {i:02}")).collect();
        let mut organizations: Vec<&str> = names.iter().map(String::as_str).collect();
        // The most common one comes first and keeps the first colour.
        organizations.extend(["Org 13", "Org 13", ""]);
        let coloring = Coloring::new(&people_in(&organizations), Kind::Organization);
        assert_eq!(coloring.entries[0].name, "Org 13");
        assert_eq!(coloring.entries[0].count, 3);
        assert_eq!(coloring.blank, 1);
        let p = theme::LIGHT;
        let (own, other) = coloring.legend(p);
        assert_eq!(own.len(), 10);
        assert_eq!(other, 4);
        let last = &coloring.entries.last().unwrap().key;
        assert_eq!(coloring.color(last, p), Some(p.muted));
        assert_eq!(coloring.color("", p), None);
    }
}
