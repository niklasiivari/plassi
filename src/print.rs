//! The data behind the A4 prints; `plassi-typeset` turns it into PDFs.

use crate::i18n::tr;
use std::collections::{BTreeMap, HashMap};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::catalog::Kind;
use crate::model::{Project, Seat, Side, TableSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrintKind {
    /// Wall chart of the tables.
    Chart,
    Kitchen,
    Bar,
    Door,
}

impl PrintKind {
    pub const ALL: [PrintKind; 4] = [
        PrintKind::Chart,
        PrintKind::Kitchen,
        PrintKind::Bar,
        PrintKind::Door,
    ];

    pub fn key(self) -> &'static str {
        match self {
            PrintKind::Chart => "chart",
            PrintKind::Kitchen => "kitchen",
            PrintKind::Bar => "bar",
            PrintKind::Door => "door",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            PrintKind::Chart => tr("Seating chart"),
            PrintKind::Kitchen => tr("Kitchen"),
            PrintKind::Bar => tr("Bar"),
            PrintKind::Door => tr("Door list"),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PrintKind::Chart => tr("Table chart for the wall."),
            PrintKind::Kitchen => tr("Table chart with meals, allergies and totals."),
            PrintKind::Bar => tr("Table chart with drinks and totals."),
            PrintKind::Door => tr("Names and prices, alphabetically."),
        }
    }
}

fn person_json(project: &Project, id: usize) -> Value {
    let a = project.person(id);
    json!({
        "price_cents": a.and_then(|a| project.attendee_price_cents(a)),
        "name": a.map(|a| a.name.as_str()).unwrap_or(""),
        "org": a.map(|a| project.category_name(Kind::Organization, a)).unwrap_or_default(),
        "group": a.map(|a| project.category_name(Kind::Ticket, a)).unwrap_or_default(),
        "food": a.map(|a| project.category_name(Kind::Food, a)).unwrap_or_default(),
        "drink": a.map(|a| project.category_name(Kind::Drink, a)).unwrap_or_default(),
        "allergies": a.map(|a| a.allergies.as_str()).unwrap_or(""),
    })
}

/// Everything the template needs, as JSON.
pub fn data(project: &Project, candidate: usize, kind: PrintKind) -> Result<Value> {
    let option = project
        .candidates
        .get(candidate)
        .ok_or_else(|| anyhow!("no seating option to print; solve first"))?;
    let tables = project.layout.rows.iter().flat_map(|r| &r.tables);
    let max_rows = tables.map(|t| t.left.max(t.right)).max().unwrap_or(1);
    let max_tables = project
        .layout
        .rows
        .iter()
        .map(|r| r.tables.len())
        .max()
        .unwrap_or(1);
    let title = if project.event.title.trim().is_empty() {
        match project.event.print_language.code() {
            "fi" => "Istumajärjestys",
            _ => "Seating",
        }
        .to_string()
    } else {
        project.event.title.clone()
    };
    let diets = (kind == PrintKind::Kitchen).then(|| Diets::new(project, &option.assignment));
    Ok(json!({
        "kind": kind.key(),
        "title": title,
        "subtitle": project.event.subtitle,
        "lang": project.event.print_language.code(),
        "rows": rows_json(project, &option.assignment, diets.as_ref()),
        "diets": diets.as_ref().map(Diets::summary),
        "people": people_json(project),
        "max_rows": max_rows,
        "max_tables": max_tables,
        "totals": {
            "food": totals_json(project, Kind::Food),
            "drink": totals_json(project, Kind::Drink),
            "group": totals_json(project, Kind::Ticket),
        },
    }))
}

/// The layout's rows and tables with each seat's occupant.
fn rows_json(
    project: &Project,
    assignment: &BTreeMap<usize, String>,
    diets: Option<&Diets>,
) -> Vec<Value> {
    let occupant: HashMap<&str, usize> = assignment.iter().map(|(p, s)| (s.as_str(), *p)).collect();
    let seats = project.layout.seats();
    project
        .layout
        .rows
        .iter()
        .map(|row| {
            let tables: Vec<Value> = row
                .tables
                .iter()
                .map(|table| table_json(project, table, &seats, &occupant, diets))
                .collect();
            json!({ "label": row.label, "tables": tables })
        })
        .collect()
}

fn table_json(
    project: &Project,
    table: &TableSpec,
    seats: &[Seat],
    occupant: &HashMap<&str, usize>,
    diets: Option<&Diets>,
) -> Value {
    let side = |side: Side, index: usize| {
        seats
            .iter()
            .find(|s| s.table_id == table.id && s.side == side && s.index == index)
            .and_then(|s| occupant.get(s.id.as_str()))
            .map(|p| {
                let mut person = person_json(project, *p);
                if let Some(diet) = diets.and_then(|d| d.seat.get(p)) {
                    person["diet"] = diet.clone();
                }
                person
            })
            .unwrap_or(Value::Null)
    };
    let length = table.left.max(table.right);
    let seat_rows: Vec<Value> = (0..length)
        .map(|i| json!({ "n": i + 1, "left": side(Side::Left, i), "right": side(Side::Right, i) }))
        .collect();
    json!({ "label": table_name(project, &table.id), "rows": seat_rows })
}

/// A table's printed name: its label, or "Table 2" / "Pöytä 2" by its place
/// in the room when it has none, so guests can tell the tables apart.
fn table_name(project: &Project, table_id: &str) -> String {
    let tables = project.layout.rows.iter().flat_map(|r| &r.tables);
    let Some((number, table)) = tables.enumerate().find(|(_, t)| t.id == table_id) else {
        return String::new();
    };
    if !table.label.trim().is_empty() {
        return table.label.clone();
    }
    let word = match project.event.print_language.code() {
        "fi" => "Pöytä",
        _ => "Table",
    };
    format!("{word} {}", number + 1)
}

/// Allergy notes longer than this go under the chart, numbered.
const LONG_ALLERGY: usize = 48;

/// What the kitchen needs at each seat: a short meal mark (none for the most
/// common meal), the allergies or a note number, and the legend and notes.
struct Diets {
    /// Per attendee ID: `{mark, allergy, note}`.
    seat: HashMap<usize, Value>,
    /// The unmarked meal and how many have it.
    plain: Value,
    /// Every other meal: `{mark, name, count}`, most common first.
    legend: Vec<Value>,
    /// Long allergy texts: `{n, name, text}`, in seating order.
    notes: Vec<Value>,
}

impl Diets {
    fn new(project: &Project, assignment: &BTreeMap<usize, String>) -> Self {
        let meals: HashMap<usize, Meal> = project
            .attendees
            .iter()
            .map(|a| (a.id, Meal::of(project, a)))
            .collect();
        let mut counts: Vec<(&Meal, usize)> = Vec::new();
        for meal in meals.values().filter(|m| !m.key.is_empty()) {
            match counts.iter_mut().find(|(m, _)| m.key == meal.key) {
                Some((_, count)) => *count += 1,
                None => counts.push((meal, 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
        let plain_key = counts
            .first()
            .map(|(m, _)| m.key.clone())
            .unwrap_or_default();
        let mut diets = Self {
            seat: HashMap::new(),
            plain: counts
                .first()
                .map(|(m, n)| json!({ "name": m.name, "count": n }))
                .unwrap_or(Value::Null),
            legend: counts
                .iter()
                .skip(1)
                .map(|(m, n)| json!({ "mark": m.mark, "name": m.name, "count": n }))
                .collect(),
            notes: Vec::new(),
        };
        for seat in project.layout.seats() {
            let Some((&id, _)) = assignment.iter().find(|(_, s)| **s == seat.id) else {
                continue;
            };
            let Some(attendee) = project.person(id) else {
                continue;
            };
            let meal = &meals[&id];
            let mark = if meal.key == plain_key {
                ""
            } else {
                meal.mark.as_str()
            };
            let allergy = allergy_text(&attendee.allergies);
            let note = (allergy.chars().count() > LONG_ALLERGY).then(|| {
                diets.notes.push(json!({
                    "n": diets.notes.len() + 1,
                    "name": attendee.name,
                    "text": allergy,
                }));
                diets.notes.len()
            });
            diets.seat.insert(
                id,
                json!({
                    "mark": mark,
                    "allergy": if note.is_some() { "" } else { allergy },
                    "note": note,
                }),
            );
        }
        diets
    }

    fn summary(&self) -> Value {
        json!({ "plain": self.plain, "legend": self.legend, "notes": self.notes })
    }
}

/// A person's meal: what groups it, its name and its print mark.
struct Meal {
    key: String,
    name: String,
    mark: String,
}

impl Meal {
    fn of(project: &Project, attendee: &crate::model::Attendee) -> Self {
        let list = project.catalog.list(Kind::Food);
        let found = project.catalog.find_all(Kind::Food, &attendee.food);
        if found.is_empty() {
            let raw = attendee.food.trim();
            return Self {
                key: crate::catalog::normalize(raw),
                name: raw.to_string(),
                mark: raw.to_string(),
            };
        }
        let names: Vec<&str> = found.iter().map(|&i| list[i].name.as_str()).collect();
        let marks: Vec<&str> = found
            .iter()
            .map(|&i| match list[i].mark.trim() {
                "" => list[i].name.as_str(),
                mark => mark,
            })
            .collect();
        Self {
            key: names.join("\u{1f}"),
            name: names.join(" / "),
            mark: marks.join(" "),
        }
    }
}

/// The allergy text, or nothing for answers that say there are none.
fn allergy_text(raw: &str) -> &str {
    let text = raw.trim();
    let none = ["ei", "ei ole", "ei mitään", "ei allergioita", "none", "no"];
    if !text.chars().any(char::is_alphabetic) || none.contains(&text.to_lowercase().as_str()) {
        ""
    } else {
        text
    }
}

/// Everyone alphabetically, for the door list.
fn people_json(project: &Project) -> Vec<Value> {
    let mut people: Vec<Value> = project
        .attendees
        .iter()
        .map(|a| person_json(project, a.id))
        .collect();
    people.sort_by(|a, b| {
        let key = |v: &Value| v["name"].as_str().unwrap_or("").to_lowercase();
        key(a).cmp(&key(b))
    });
    people
}

/// How many attendees have each value of `field`, most common first.
/// Count per category (unmapped spellings count on their own).
fn totals_json(project: &Project, kind: Kind) -> Vec<Value> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for a in &project.attendees {
        let mut names = project.category_names(kind, a);
        if names.is_empty() {
            let raw = kind.raw(a).trim();
            names.push(if raw.is_empty() {
                "—".into()
            } else {
                raw.to_string()
            });
        }
        for name in names {
            *counts.entry(name).or_default() += 1;
        }
    }
    let mut list: Vec<_> = counts.into_iter().collect();
    list.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    list.into_iter()
        .map(|(k, n)| json!({ "label": k, "count": n }))
        .collect()
}

/// A print as PDF (on the desktop; the browser typesets through its own module).
#[cfg(not(target_arch = "wasm32"))]
pub fn render(project: &Project, candidate: usize, kind: PrintKind) -> Result<Vec<u8>> {
    plassi_typeset::pdf(&data(project, candidate, kind)?.to_string())
        .map_err(|error| anyhow!(error))
}

/// File name of a print, e.g. "plassi-kitchen.pdf".
pub fn file_name(kind: PrintKind) -> String {
    format!("plassi-{}.pdf", kind.key())
}

#[cfg(not(target_arch = "wasm32"))]
/// Writes the PDF next to the project file and returns its path.
pub fn export(
    project: &Project,
    project_path: &std::path::Path,
    candidate: usize,
    kind: PrintKind,
) -> Result<std::path::PathBuf> {
    let pdf = render(project, candidate, kind)?;
    let dir = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let path = dir.join(file_name(kind));
    std::fs::write(&path, pdf)?;
    Ok(path)
}
