//! The data behind the A4 prints; `plassi-typeset` turns it into PDFs.

use crate::i18n::tr;
use std::collections::{BTreeMap, HashMap};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::catalog::Kind;
use crate::model::{Project, Seat, Side, TableSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrintKind {
    /// Wall chart plus an alphabetical "find your seat" index.
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
            PrintKind::Kitchen => tr("Kitchen list"),
            PrintKind::Bar => tr("Bar list"),
            PrintKind::Door => tr("Door list"),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            PrintKind::Chart => tr(
                "For the wall: every table with names, plus an alphabetical index to find your seat.",
            ),
            PrintKind::Kitchen => tr("Food choices and allergies per table and seat, with totals."),
            PrintKind::Bar => tr("Drink choices per table and seat, with totals."),
            PrintKind::Door => {
                tr("Alphabetical ticket check with price category, table and a tick box.")
            }
        }
    }
}

fn person_json(project: &Project, id: usize) -> Value {
    let a = project.person(id);
    json!({
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
        "Seating".to_string()
    } else {
        project.event.title.clone()
    };
    Ok(json!({
        "kind": kind.key(),
        "title": title,
        "subtitle": project.event.subtitle,
        "lang": project.event.print_language.code(),
        "rows": rows_json(project, &option.assignment),
        "people": people_json(project, &option.assignment),
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
fn rows_json(project: &Project, assignment: &BTreeMap<usize, String>) -> Vec<Value> {
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
                .map(|table| table_json(project, table, &seats, &occupant))
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
) -> Value {
    let side = |side: Side, index: usize| {
        seats
            .iter()
            .find(|s| s.table_id == table.id && s.side == side && s.index == index)
            .and_then(|s| occupant.get(s.id.as_str()))
            .map(|p| person_json(project, *p))
            .unwrap_or(Value::Null)
    };
    let length = table.left.max(table.right);
    let seat_rows: Vec<Value> = (0..length)
        .map(|i| json!({ "n": i + 1, "left": side(Side::Left, i), "right": side(Side::Right, i) }))
        .collect();
    json!({ "label": table.label, "rows": seat_rows })
}

/// Everyone alphabetically, with table and a short place code such as "L3".
fn people_json(project: &Project, assignment: &BTreeMap<usize, String>) -> Vec<Value> {
    let mut people: Vec<Value> = project
        .attendees
        .iter()
        .map(|a| {
            let mut value = person_json(project, a.id);
            let place = assignment.get(&a.id).and_then(|s| project.seat_place(s));
            let (table, code) = match place {
                Some((table, Side::Left, row)) => (table, format!("L{row}")),
                Some((table, Side::Right, row)) => (table, format!("R{row}")),
                None => ("", String::new()),
            };
            value["table"] = json!(table);
            value["place"] = json!(code);
            value
        })
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
