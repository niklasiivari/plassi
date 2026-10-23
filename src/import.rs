//! Attendee import from spreadsheet files or pasted rows.
//!
//! The template's column names ([`Column::TEMPLATE`], English or Finnish) are
//! recognised exactly, and every sheet's column mapping can be edited before
//! applying. Unknown columns are kept per attendee in `extra`. Imports can add
//! distinct attendees or update matching names while preserving rules, layout
//! and seating options.

mod workbook;
pub use workbook::read_sheets;

use crate::catalog::normalize;
use crate::i18n::{fill, tr};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context;
use anyhow::{Result, bail};

use crate::model::{
    Attendee, ImportField, ImportNote, Importance, Project, Rule, RuleKind, Target, WishKind,
};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum Column {
    Name,
    FirstName,
    LastName,
    Organization,
    Group,
    Avec,
    Wish,
    Food,
    Drink,
    Allergies,
    /// Kept per attendee as extra info.
    Other,
    /// Left out of the import.
    Ignore,
}

/// How a column's meaning was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// The header is a template column name.
    Exact,
    /// Chosen by the person importing on an earlier import of this project.
    Remembered,
    /// Not a template column; kept as extra info unless the person chooses.
    Unknown,
}

impl Column {
    /// The import template's columns, in order.
    pub const TEMPLATE: [Column; 8] = [
        Column::Name,
        Column::Organization,
        Column::Group,
        Column::Avec,
        Column::Wish,
        Column::Food,
        Column::Drink,
        Column::Allergies,
    ];

    /// Everything a column can be set to when importing.
    pub const CHOICES: [Column; 12] = [
        Column::Name,
        Column::FirstName,
        Column::LastName,
        Column::Organization,
        Column::Group,
        Column::Avec,
        Column::Wish,
        Column::Food,
        Column::Drink,
        Column::Allergies,
        Column::Other,
        Column::Ignore,
    ];

    /// The template header for this column: (English, Finnish).
    pub fn headers(self) -> (&'static str, &'static str) {
        match self {
            Column::Name => ("Name", "Nimi"),
            Column::FirstName => ("First name", "Etunimi"),
            Column::LastName => ("Last name", "Sukunimi"),
            Column::Organization => ("Organization", "Järjestö"),
            Column::Group => ("Participant group", "Osallistujaryhmä"),
            Column::Avec => ("Avec", "Avec"),
            Column::Wish => ("Seating wish", "Pöytäseuratoive"),
            Column::Food => ("Food", "Ruoka"),
            Column::Drink => ("Drinks", "Juoma"),
            Column::Allergies => ("Allergies", "Allergiat"),
            Column::Other | Column::Ignore => ("", ""),
        }
    }

    /// The template header in the interface language.
    pub fn header(self) -> &'static str {
        let (en, fi) = self.headers();
        match crate::i18n::current() {
            crate::i18n::Lang::Fi => fi,
            crate::i18n::Lang::En => en,
        }
    }

    /// What goes in the column, for the template help.
    pub fn explain(self) -> &'static str {
        match self {
            Column::Name => tr("Full name. Used to match existing people."),
            Column::Organization => tr("Organization or club."),
            Column::Group => tr("Ticket type or participant group."),
            Column::Avec => tr("Avec's name, as written."),
            Column::Wish => tr("Seating wishes, as written."),
            Column::Food => tr("Menu choice."),
            Column::Drink => tr("Drink choice."),
            Column::Allergies => tr("Allergies and diets."),
            _ => "",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Column::Name => tr("Name"),
            Column::FirstName => tr("First name"),
            Column::LastName => tr("Last name"),
            Column::Organization => tr("Organization"),
            Column::Group => tr("Participant group"),
            Column::Avec => tr("Avec"),
            Column::Wish => tr("Seating wish"),
            Column::Food => tr("Food"),
            Column::Drink => tr("Drinks"),
            Column::Allergies => tr("Allergies / diet"),
            Column::Other => tr("Kept as extra info"),
            Column::Ignore => tr("Ignore"),
        }
    }
}

/// A header's column: a template name in either language (any case), else
/// unknown until the person importing chooses. There is no guessing.
pub fn recognise(header: &str) -> (Column, Match) {
    let h = header.trim().to_lowercase();
    let exact = Column::CHOICES.into_iter().find(|c| {
        let (en, fi) = c.headers();
        !en.is_empty() && (h == en.to_lowercase() || h == fi.to_lowercase())
    });
    match exact {
        Some(column) => (column, Match::Exact),
        None => (Column::Other, Match::Unknown),
    }
}

pub struct Sheet {
    pub headers: Vec<String>,
    /// What each column is imported as; the importer may change these.
    pub columns: Vec<Column>,
    /// How each column's meaning was found.
    pub matches: Vec<Match>,
    pub rows: Vec<Vec<String>>,
}

/// Whether another sheet adds distinct attendees or updates existing names.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ImportMode {
    #[default]
    Add,
    UpdateMatching,
}

impl Sheet {
    /// Applies the column choices made on earlier imports into this project.
    pub fn remember(&mut self, choices: &BTreeMap<String, Column>) {
        for ((header, column), found) in self
            .headers
            .iter()
            .zip(&mut self.columns)
            .zip(&mut self.matches)
        {
            if *found == Match::Unknown
                && let Some(choice) = choices.get(&normalize(header))
            {
                *column = *choice;
                *found = Match::Remembered;
            }
        }
    }

    /// Whether the columns give each row a name.
    pub fn has_name(&self) -> bool {
        self.columns.contains(&Column::Name)
            || (self.columns.contains(&Column::FirstName)
                && self.columns.contains(&Column::LastName))
    }
}

/// Parses tab-, semicolon- or comma-separated text with a header row.
pub fn parse_sheet(text: &str) -> Result<Sheet> {
    // A byte-order mark from a saved file isn't part of the first header.
    let text = text.trim_start_matches('\u{feff}');
    // Parse the header with each separator so punctuation inside quoted
    // headers does not get mistaken for additional columns.
    let delimiter = b"\t;,"
        .iter()
        .copied()
        .map(|d| {
            let fields = csv::ReaderBuilder::new()
                .delimiter(d)
                .from_reader(text.trim_start().as_bytes())
                .headers()
                .map_or(0, |headers| headers.len());
            (fields, d)
        })
        .max_by_key(|(count, _)| *count)
        .map_or(b'\t', |(_, d)| d);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(text.trim_start().as_bytes());
    let headers: Vec<String> = reader
        .headers()?
        .iter()
        .map(|h| h.trim().to_string())
        .collect();
    let (columns, matches) = headers.iter().map(|h| recognise(h)).unzip();
    let rows: Vec<Vec<String>> = reader
        .records()
        .collect::<csv::Result<Vec<_>>>()?
        .into_iter()
        .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
        .filter(|r: &Vec<String>| r.iter().any(|c| !c.is_empty()))
        .collect();
    Ok(Sheet {
        headers,
        columns,
        matches,
        rows,
    })
}

struct Row {
    name: String,
    organization: Option<String>,
    group: Option<String>,
    avec: String,
    wish: String,
    food: Option<String>,
    drink: Option<String>,
    allergies: Option<String>,
    extra: BTreeMap<String, String>,
}

fn rows(sheet: &Sheet) -> Vec<Row> {
    sheet
        .rows
        .iter()
        .map(|cells| {
            let get = |col: Column| -> Option<String> {
                let parts: Vec<&str> = sheet
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c == col)
                    .filter_map(|(i, _)| cells.get(i).map(String::as_str))
                    .filter(|v| !v.is_empty())
                    .collect();
                sheet.columns.contains(&col).then(|| parts.join("; "))
            };
            let name = get(Column::Name)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "{} {}",
                        get(Column::FirstName).unwrap_or_default(),
                        get(Column::LastName).unwrap_or_default()
                    )
                    .trim()
                    .to_string()
                });
            let extra = sheet
                .columns
                .iter()
                .enumerate()
                .filter(|(_, c)| **c == Column::Other)
                .filter_map(|(i, _)| {
                    let value = cells.get(i)?;
                    (!value.is_empty()).then(|| (sheet.headers[i].clone(), value.clone()))
                })
                .collect();
            Row {
                name,
                organization: get(Column::Organization),
                group: get(Column::Group),
                avec: get(Column::Avec).unwrap_or_default(),
                wish: get(Column::Wish).unwrap_or_default(),
                food: get(Column::Food),
                drink: get(Column::Drink),
                allergies: get(Column::Allergies),
                extra,
            }
        })
        .filter(|r| !r.name.is_empty())
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn import_tsv(path: &Path) -> Result<Project> {
    let text = std::fs::read_to_string(path)
        .with_context(|| fill("Could not open {}", &[&path.display()]))?;
    let sheet = parse_sheet(&text)?;
    let mut project = Project::new();
    merge(&mut project, &sheet)?;
    Ok(project)
}

#[derive(Default, Debug)]
pub struct MergeSummary {
    pub added: usize,
    pub updated: usize,
    pub new_notes: usize,
    pub new_rules: usize,
    /// Names that appear more than once in the sheet; each row became its own person.
    pub duplicate_names: Vec<String>,
}

/// Adds or updates attendees from a sheet, matching by name. Existing rules,
/// layout and options are kept; new sign-up text becomes unreviewed notes.
#[cfg(not(target_arch = "wasm32"))]
pub fn merge(project: &mut Project, sheet: &Sheet) -> Result<MergeSummary> {
    let (next, summary) = prepare(project, sheet, ImportMode::UpdateMatching)?;
    *project = next;
    Ok(summary)
}

/// Prepares an entire import without changing the current project. The same
/// result can be previewed, then applied as one undoable edit.
pub fn prepare(
    project: &Project,
    sheet: &Sheet,
    mode: ImportMode,
) -> Result<(Project, MergeSummary)> {
    let mut next = project.clone();
    let summary = apply(&mut next, sheet, mode)?;
    Ok((next, summary))
}

fn apply(project: &mut Project, sheet: &Sheet, mode: ImportMode) -> Result<MergeSummary> {
    if !sheet.has_name() {
        bail!(fill(
            "Choose a “{}” column (found: {})",
            &[&Column::Name.label(), &sheet.headers.join(", ")]
        ));
    }
    // Remember choices for columns outside the template, for the next import.
    for ((header, column), found) in sheet.headers.iter().zip(&sheet.columns).zip(&sheet.matches) {
        if *found != Match::Exact {
            project.import_columns.insert(normalize(header), *column);
        }
    }
    let rows = rows(sheet);
    if rows.is_empty() {
        bail!(tr("No people in the sheet"));
    }
    let mut summary = MergeSummary::default();
    let mut attendees = AttendeeIndex::new(project);
    let ids: Vec<usize> = rows
        .iter()
        .map(|row| upsert_attendee(project, row, mode, &mut attendees, &mut summary))
        .collect();
    let mut name_map: HashMap<String, Vec<usize>> = HashMap::new();
    for p in &project.attendees {
        name_map.entry(normalize(&p.name)).or_default().push(p.id);
    }
    let mut notes = NoteIndex::new(project);
    for (row, &person) in rows.iter().zip(&ids) {
        for (field, text) in [
            (ImportField::Avec, &row.avec),
            (ImportField::Wish, &row.wish),
        ] {
            add_note(
                project,
                &name_map,
                &mut notes,
                person,
                field,
                text.trim(),
                &mut summary,
            );
        }
    }
    project.validate()?;
    Ok(summary)
}

/// Existing same-named attendees stay in their original order. A row claims
/// one of them at most once, including when several spellings normalize alike.
struct AttendeeIndex {
    unmatched: HashMap<String, VecDeque<usize>>,
    claimed_names: HashSet<String>,
    next_id: usize,
}

impl AttendeeIndex {
    fn new(project: &Project) -> Self {
        let mut unmatched: HashMap<String, VecDeque<usize>> = HashMap::new();
        for (index, attendee) in project.attendees.iter().enumerate() {
            unmatched
                .entry(normalize(&attendee.name))
                .or_default()
                .push_back(index);
        }
        Self {
            unmatched,
            claimed_names: HashSet::new(),
            next_id: project
                .attendees
                .iter()
                .map(|a| a.id.saturating_add(1))
                .max()
                .unwrap_or(0),
        }
    }
}

/// Deduplication and ID allocation shared by every note in one import.
struct NoteIndex {
    // The boolean distinguishes the two ImportField variants.
    texts: HashMap<(usize, bool), HashSet<String>>,
    next_rule_id: u64,
}

impl NoteIndex {
    fn new(project: &Project) -> Self {
        let mut texts: HashMap<(usize, bool), HashSet<String>> = HashMap::new();
        for note in &project.notes {
            texts
                .entry((note.person, note.field == ImportField::Avec))
                .or_default()
                .insert(note.text.clone());
        }
        Self {
            texts,
            next_rule_id: project
                .rules
                .iter()
                .map(|rule| rule.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        }
    }
}

/// Updates the attendee with the row's name, or adds one; returns their ID.
/// Each existing attendee matches at most one row (`claimed`), so two rows
/// with the same name stay two people.
fn upsert_attendee(
    project: &mut Project,
    row: &Row,
    mode: ImportMode,
    attendees: &mut AttendeeIndex,
    summary: &mut MergeSummary,
) -> usize {
    let name = normalize(&row.name);
    if !attendees.claimed_names.insert(name.clone()) {
        summary.duplicate_names.push(row.name.clone());
    }
    let existing = (mode == ImportMode::UpdateMatching)
        .then(|| {
            attendees
                .unmatched
                .get_mut(&name)
                .and_then(VecDeque::pop_front)
        })
        .flatten();
    let index = match existing {
        Some(index) => {
            summary.updated += 1;
            index
        }
        None => {
            let id = attendees.next_id;
            attendees.next_id = attendees.next_id.saturating_add(1);
            project.attendees.push(Attendee {
                id,
                name: row.name.clone(),
                organization: String::new(),
                participant_group: String::new(),
                tags: Vec::new(),
                food: String::new(),
                drink: String::new(),
                allergies: String::new(),
                price_override_cents: None,
                extra: BTreeMap::new(),
            });
            summary.added += 1;
            project.attendees.len() - 1
        }
    };
    let a = &mut project.attendees[index];
    for (field, value) in [
        (&mut a.organization, &row.organization),
        (&mut a.participant_group, &row.group),
        (&mut a.food, &row.food),
        (&mut a.drink, &row.drink),
        (&mut a.allergies, &row.allergies),
    ] {
        if let Some(value) = value {
            *field = value.clone();
        }
    }
    a.extra.extend(row.extra.clone());
    a.id
}

/// Records a new avec/wish note; if it names exactly one other attendee,
/// also adds the wish rule and marks the note reviewed.
fn add_note(
    project: &mut Project,
    name_map: &HashMap<String, Vec<usize>>,
    notes: &mut NoteIndex,
    person: usize,
    field: ImportField,
    text: &str,
    summary: &mut MergeSummary,
) {
    if text.is_empty()
        || !notes
            .texts
            .entry((person, field == ImportField::Avec))
            .or_default()
            .insert(text.to_string())
    {
        return;
    }
    let exact = name_map
        .get(&normalize(text))
        .and_then(|ids| (ids.len() == 1).then_some(ids[0]))
        .filter(|id| *id != person);
    if let Some(target) = exact {
        project.rules.push(wish_rule(
            notes.next_rule_id,
            person,
            vec![target],
            field.wish(),
            text,
        ));
        notes.next_rule_id = notes.next_rule_id.saturating_add(1);
        summary.new_rules += 1;
    }
    project.notes.push(ImportNote {
        person,
        field,
        text: text.into(),
        reviewed: exact.is_some(),
    });
    summary.new_notes += 1;
}

/// An avec or seating wish of normal importance.
pub fn wish_rule(
    id: u64,
    requester: usize,
    targets: Vec<usize>,
    wish: WishKind,
    source: &str,
) -> Rule {
    Rule {
        id,
        label: wish.label().into(),
        source: source.into(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester,
            target: Target::People { ids: targets },
            wish,
            importance: Importance::Normal,
            max_class: None,
        },
    }
}

/// The template's header row, tab-separated for pasting into a spreadsheet.
pub fn header_row() -> String {
    Column::TEMPLATE.map(Column::header).join("\t")
}

/// A template file: header row and one example row, semicolon-separated as
/// spreadsheet programs expect in most European locales, with a byte-order
/// mark so they read it as UTF-8.
pub fn template_csv() -> String {
    let example = [
        "X Y",
        "ABC",
        tr("Member"),
        "Z W",
        tr("With Q R"),
        tr("Vegetarian"),
        tr("Water"),
        tr("Nuts"),
    ];
    csv_text(&[
        Column::TEMPLATE.map(Column::header).to_vec(),
        example.to_vec(),
    ])
}

/// Everyone in the project in the template's columns, for editing in a
/// spreadsheet and importing back.
pub fn export_csv(project: &Project) -> String {
    let note = |person: usize, field: ImportField| {
        project
            .notes
            .iter()
            .filter(|n| n.person == person && n.field == field)
            .map(|n| n.text.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    let mut rows = vec![Column::TEMPLATE.map(|c| c.header().to_string()).to_vec()];
    for a in &project.attendees {
        rows.push(vec![
            a.name.clone(),
            a.organization.clone(),
            a.participant_group.clone(),
            note(a.id, ImportField::Avec),
            note(a.id, ImportField::Wish),
            a.food.clone(),
            a.drink.clone(),
            a.allergies.clone(),
        ]);
    }
    csv_text(&rows)
}

fn csv_text<S: AsRef<str>>(rows: &[Vec<S>]) -> String {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(Vec::new());
    for row in rows {
        let _ = writer.write_record(row.iter().map(AsRef::as_ref));
    }
    let bytes = writer.into_inner().unwrap_or_default();
    format!("\u{feff}{}", String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attendee(id: usize, name: &str) -> Attendee {
        Attendee {
            id,
            name: name.into(),
            organization: String::new(),
            participant_group: String::new(),
            tags: Vec::new(),
            food: String::new(),
            drink: String::new(),
            allergies: String::new(),
            price_override_cents: None,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn indexed_import_matches_same_names_once_in_attendee_order() {
        let mut project = Project::new();
        project.attendees = vec![
            attendee(42, "Person 1"),
            attendee(3, "PERSON 1"),
            attendee(19, "Person 2"),
        ];
        let before = serde_json::to_value(&project).unwrap();
        let sheet = parse_sheet(
            "Name\tFood\nPerson 1\tABC\nPERSON 1\tXYZ\nPerson 1\tABC / XYZ\nPerson 2\tXYZ",
        )
        .unwrap();
        let (next, summary) = prepare(&project, &sheet, ImportMode::UpdateMatching).unwrap();
        assert_eq!((summary.added, summary.updated), (1, 3));
        assert_eq!(summary.duplicate_names, ["PERSON 1", "Person 1"]);
        let actual: Vec<_> = next
            .attendees
            .iter()
            .map(|a| (a.id, a.name.as_str(), a.food.as_str()))
            .collect();
        assert_eq!(
            actual,
            [
                (42, "Person 1", "ABC"),
                (3, "PERSON 1", "XYZ"),
                (19, "Person 2", "XYZ"),
                (43, "Person 1", "ABC / XYZ"),
            ]
        );
        assert_eq!(serde_json::to_value(&project).unwrap(), before);
    }

    #[test]
    fn indexed_add_keeps_existing_names_and_reports_only_repeated_rows() {
        let mut project = Project::new();
        project.attendees = vec![attendee(42, "Person 1")];
        let sheet = parse_sheet("Name\tFood\nPERSON 1\tABC\nPerson 1\tXYZ").unwrap();
        let (next, summary) = prepare(&project, &sheet, ImportMode::Add).unwrap();
        assert_eq!((summary.added, summary.updated), (2, 0));
        assert_eq!(summary.duplicate_names, ["Person 1"]);
        assert_eq!(next.attendees[0].food, "");
        assert_eq!(next.attendees[1].id, 43);
        assert_eq!(next.attendees[1].food, "ABC");
        assert_eq!(next.attendees[2].id, 44);
        assert_eq!(next.attendees[2].food, "XYZ");
    }

    #[test]
    fn indexed_notes_keep_fields_separate_and_allocate_after_existing_rules() {
        let mut project = Project::new();
        project.attendees = vec![attendee(3, "Person 1"), attendee(42, "Person 2")];
        project.notes.push(ImportNote {
            person: 3,
            field: ImportField::Avec,
            text: "Person 2".into(),
            reviewed: true,
        });
        project
            .rules
            .push(wish_rule(99, 3, vec![42], WishKind::Avec, "Person 2"));
        let sheet = parse_sheet(
            "Name\tAvec\tSeating wish\nPerson 1\tPerson 2\tPerson 2\nPerson 2\t\tPerson 1",
        )
        .unwrap();
        let (next, summary) = prepare(&project, &sheet, ImportMode::UpdateMatching).unwrap();
        assert_eq!((summary.new_notes, summary.new_rules), (2, 2));
        assert_eq!(next.notes.len(), 3);
        assert_eq!(next.notes[0].field, ImportField::Avec);
        assert_eq!(next.notes[1].field, ImportField::Wish);
        assert!(next.notes.iter().all(|n| n.reviewed));
        assert_eq!(
            next.rules.iter().map(|r| r.id).collect::<Vec<_>>(),
            [99, 100, 101]
        );
        let (again, repeated) = prepare(&next, &sheet, ImportMode::UpdateMatching).unwrap();
        assert_eq!((repeated.new_notes, repeated.new_rules), (0, 0));
        assert_eq!(
            serde_json::to_value(&again).unwrap(),
            serde_json::to_value(&next).unwrap()
        );
    }

    #[test]
    fn indexed_id_allocation_leaves_invalid_projects_to_validation() {
        let mut project = Project::new();
        project.attendees = vec![attendee(0, "Person 1"), attendee(1, "Person 2")];
        project
            .rules
            .push(wish_rule(u64::MAX, 0, vec![1], WishKind::Seating, ""));
        let sheet = parse_sheet("Name\nPerson 1").unwrap();
        assert!(prepare(&project, &sheet, ImportMode::UpdateMatching).is_err());

        project.rules.clear();
        project.attendees[1].id = usize::MAX;
        assert!(prepare(&project, &sheet, ImportMode::Add).is_err());
    }
}
