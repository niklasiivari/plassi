//! The data behind the A4 prints; `plassi-typeset` turns it into PDFs.

use crate::i18n::tr;
use std::collections::{BTreeMap, HashMap};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::catalog::Kind;
use crate::model::{Project, Seat, Side, TableShape, TableSpec};

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
    let title = if project.event.title.trim().is_empty() {
        match project.event.print_language.code() {
            "fi" => "Istumajärjestys",
            _ => "Seating",
        }
        .to_string()
    } else {
        project.event.title.clone()
    };
    let marks = match kind {
        PrintKind::Kitchen => Some(Marks::new(project, &option.assignment, Kind::Food)),
        PrintKind::Bar => Some(Marks::new(project, &option.assignment, Kind::Drink)),
        _ => None,
    };
    Ok(json!({
        "kind": kind.key(),
        "title": title,
        "subtitle": project.event.subtitle,
        "lang": project.event.print_language.code(),
        "rows": rows_json(project, &option.assignment, marks.as_ref()),
        "marks": marks.as_ref().map(Marks::summary),
        "people": people_json(project),
        "max_rows": max_rows,
        "room_map": room_map(project, &option.assignment, marks.as_ref()),
    }))
}

/// How far a printed name reaches out from its seat, in room units: the gap
/// to the seat and the name itself.
const NAME_GAP: f32 = 16.0;
const NAME_WIDTH: f32 = 190.0;

/// The room as it stands, with each seat's occupant beside it, for layouts
/// a grid of long tables cannot show: placed freely or with round tables.
/// With `marks`, each seat also carries its meal or drink mark and allergies.
fn room_map(
    project: &Project,
    assignment: &BTreeMap<usize, String>,
    marks: Option<&Marks>,
) -> Option<Value> {
    let layout = &project.layout;
    let tables = || layout.rows.iter().flat_map(|r| &r.tables);
    if !layout.free_layout && tables().all(|t| t.shape == TableShape::Long) {
        return None;
    }
    let frames = layout.table_frames();
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut include = |x: f32, y: f32, w: f32, h: f32| {
        min_x = min_x.min(x - w / 2.0);
        min_y = min_y.min(y - h / 2.0);
        max_x = max_x.max(x + w / 2.0);
        max_y = max_y.max(y + h / 2.0);
    };
    let tables: Vec<_> = tables().map(|table| {
        let at = frames[&table.id];
        let (w, h) = table.half_size();
        let h = if table.shape == TableShape::Long { h + 28.0 } else { h };
        json!({ "x": at.x, "y": at.y, "w": w * 2.0, "h": h * 2.0, "rotation": at.rotation, "round": table.shape == TableShape::Round, "label": table_name(project, &table.id) })
    }).collect();
    let occupant: HashMap<&str, usize> = assignment.iter().map(|(p, s)| (s.as_str(), *p)).collect();
    let seats: Vec<_> = layout
        .seats()
        .iter()
        .map(|seat| {
            let (dy, dx) = seat.out.sin_cos();
            let reach = NAME_GAP + NAME_WIDTH;
            include(seat.x + dx * reach, seat.y + dy * reach, 60.0, 60.0);
            include(seat.x, seat.y, 60.0, 60.0);
            let person = occupant.get(seat.id.as_str()).copied();
            let name = person.and_then(|p| project.person(p)).map(|a| a.name.as_str());
            let service = person.and_then(|p| marks.and_then(|m| m.seat.get(&p)));
            json!({ "x": seat.x, "y": seat.y, "dx": dx, "dy": dy, "name": name, "service": service })
        })
        .collect();
    for label in &layout.labels {
        include(label.x, label.y, 300.0, 150.0);
    }
    if let Some(room) = layout.room {
        include(
            room.x + room.width / 2.0,
            room.y + room.height / 2.0,
            room.width,
            room.height,
        );
    }
    Some(json!({
        "x": min_x - 20.0, "y": min_y - 20.0, "w": max_x - min_x + 40.0, "h": max_y - min_y + 40.0,
        "tables": tables, "seats": seats, "labels": layout.labels, "walls": layout.room,
        "name_gap": NAME_GAP, "name_width": NAME_WIDTH, "line": label_line(&layout.seats()),
    }))
}

/// Height of a printed name line, in room units: as tall as the closest
/// neighbouring names at one table allow, which at a round table of many
/// seats is less than a seat pitch. Names reach sideways from seats facing
/// sideways and up or down from the rest, as the template draws them.
fn label_line(seats: &[Seat]) -> f32 {
    let mut groups: HashMap<(&str, u8), Vec<f32>> = HashMap::new();
    for seat in seats {
        let (dy, dx) = seat.out.sin_cos();
        let (x, y) = (seat.x + dx * NAME_GAP, seat.y + dy * NAME_GAP);
        let (way, across) = match (dx, dy) {
            (dx, _) if dx > 0.2 => (0, y),
            (dx, _) if dx < -0.2 => (1, y),
            (_, dy) if dy < 0.0 => (2, x),
            _ => (3, x),
        };
        groups
            .entry((seat.table_id.as_str(), way))
            .or_default()
            .push(across);
    }
    let closest = groups
        .values_mut()
        .flat_map(|places| {
            places.sort_by(f32::total_cmp);
            places.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>()
        })
        .fold(f32::INFINITY, f32::min);
    closest.clamp(24.0, 44.0)
}

/// The layout's rows and tables with each seat's occupant.
fn rows_json(
    project: &Project,
    assignment: &BTreeMap<usize, String>,
    marks: Option<&Marks>,
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
                .map(|table| table_json(project, table, &seats, &occupant, marks))
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
    marks: Option<&Marks>,
) -> Value {
    let side = |side: Side, index: usize| {
        seats
            .iter()
            .find(|s| s.table_id == table.id && s.side == side && s.index == index)
            .and_then(|s| occupant.get(s.id.as_str()))
            .map(|p| {
                let mut person = person_json(project, *p);
                if let Some(mark) = marks.and_then(|m| m.seat.get(p)) {
                    person["service"] = mark.clone();
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

/// A table's printed name: its label, or "Table 2" in the print language by its place
/// in the room when it has none, so guests can tell the tables apart.
pub(crate) fn table_name(project: &Project, table_id: &str) -> String {
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

/// What the kitchen or bar needs at each seat: a short mark for the meal or
/// drink (none for the most common one), the allergies or a note number,
/// and the legend and notes.
struct Marks {
    /// Per attendee ID: `{mark, allergy, note}`.
    seat: HashMap<usize, Value>,
    /// The unmarked choice and how many have it.
    plain: Value,
    /// Every other choice: `{mark, name, count}`, most common first.
    legend: Vec<Value>,
    /// Long allergy texts: `{n, name, text}`, in seating order.
    notes: Vec<Value>,
}

impl Marks {
    /// For `kind` food or drinks.
    fn new(project: &Project, assignment: &BTreeMap<usize, String>, kind: Kind) -> Self {
        let choices: HashMap<usize, Choice> = project
            .attendees
            .iter()
            .map(|a| (a.id, Choice::of(project, a, kind)))
            .collect();
        let mut counts: Vec<(&Choice, usize)> = Vec::new();
        for choice in choices.values().filter(|c| !c.key.is_empty()) {
            match counts.iter_mut().find(|(c, _)| c.key == choice.key) {
                Some((_, count)) => *count += 1,
                None => counts.push((choice, 1)),
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
        let plain_key = counts
            .first()
            .map(|(c, _)| c.key.clone())
            .unwrap_or_default();
        let mut marks = Self {
            seat: HashMap::new(),
            plain: counts
                .first()
                .map(|(c, n)| json!({ "name": c.name, "count": n }))
                .unwrap_or(Value::Null),
            legend: counts
                .iter()
                .skip(1)
                .map(|(c, n)| json!({ "mark": c.mark, "name": c.name, "count": n }))
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
            let choice = &choices[&id];
            let mark = if choice.key == plain_key {
                ""
            } else {
                choice.mark.as_str()
            };
            let allergy = allergy_text(&attendee.allergies);
            let note = (allergy.chars().count() > LONG_ALLERGY).then(|| {
                marks.notes.push(json!({
                    "n": marks.notes.len() + 1,
                    "name": attendee.name,
                    "text": allergy,
                }));
                marks.notes.len()
            });
            marks.seat.insert(
                id,
                json!({
                    "mark": mark,
                    "allergy": if note.is_some() { "" } else { allergy },
                    "note": note,
                }),
            );
        }
        marks
    }

    fn summary(&self) -> Value {
        json!({ "plain": self.plain, "legend": self.legend, "notes": self.notes })
    }
}

/// A person's meal or drink: what groups it, its name and its print mark.
struct Choice {
    key: String,
    name: String,
    mark: String,
}

impl Choice {
    fn of(project: &Project, attendee: &crate::model::Attendee, kind: Kind) -> Self {
        let list = project.catalog.list(kind);
        let raw = kind.raw(attendee).trim();
        let found = project.catalog.find_all(kind, raw);
        if found.is_empty() {
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
pub(crate) fn allergy_text(raw: &str) -> &str {
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

/// A print as PDF (on the desktop; the browser typesets through its own module).
#[cfg(not(target_arch = "wasm32"))]
pub fn render(project: &Project, candidate: usize, kind: PrintKind) -> Result<Vec<u8>> {
    plassi_typeset::pdf(&data(project, candidate, kind)?.to_string())
        .map_err(|error| anyhow!(error))
}

/// File name of a print in the print language, after the project, e.g.
/// "<project> - kitchen.pdf", so it stays recognisable when sent on.
pub fn file_name(project: &Project, project_name: &str, kind: PrintKind) -> String {
    let word = match (project.event.print_language.code(), kind) {
        ("fi", PrintKind::Chart) => "istumajärjestys",
        ("fi", PrintKind::Kitchen) => "keittiö",
        ("fi", PrintKind::Bar) => "baari",
        ("fi", PrintKind::Door) => "ovilista",
        (_, PrintKind::Chart) => "seating",
        (_, PrintKind::Kitchen) => "kitchen",
        (_, PrintKind::Bar) => "bar",
        (_, PrintKind::Door) => "door list",
    };
    format!("{project_name} - {word}.pdf")
}

/// Bundle PDFs into a single download without recompressing their streams.
#[cfg(not(target_arch = "wasm32"))]
pub fn pdf_archive(files: &[(String, &[u8])]) -> Result<Vec<u8>> {
    use std::io::{Cursor, Write};
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in files {
        archive.start_file(name, options)?;
        archive.write_all(bytes)?;
    }
    Ok(archive.finish()?.into_inner())
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
    let name = project_path
        .file_stem()
        .map_or("plassi".into(), |s| s.to_string_lossy());
    let path = dir.join(file_name(project, &name, kind));
    std::fs::write(&path, pdf)?;
    Ok(path)
}

#[cfg(test)]
mod archive_tests {
    #[test]
    fn batch_download_preserves_every_pdf_name_and_content() {
        use std::io::{Cursor, Read};
        let files = [
            ("X - seating.pdf".to_owned(), b"%PDF-chart".as_slice()),
            ("X - kitchen.pdf".to_owned(), b"%PDF-kitchen".as_slice()),
            ("X - bar.pdf".to_owned(), b"%PDF-bar".as_slice()),
            ("X - door list.pdf".to_owned(), b"%PDF-door".as_slice()),
        ];
        let bytes = super::pdf_archive(&files).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), files.len());
        for (name, expected) in files {
            let mut actual = Vec::new();
            archive
                .by_name(&name)
                .unwrap()
                .read_to_end(&mut actual)
                .unwrap();
            assert_eq!(actual, expected);
        }
    }
}
