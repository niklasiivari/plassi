//! Attendee import from TSV files or spreadsheet text pasted from Excel.
//!
//! Columns are recognised by header name (Finnish or English). Unknown columns
//! are kept per attendee in `extra`. Importing can create a fresh project or
//! merge into an existing one, matching attendees by name so rules, layout and
//! options survive a re-import with more columns.

use crate::i18n::{fill, tr};
use crate::catalog::normalize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::model::{
    Attendee, ImportField, ImportNote, Project, ProximityCosts, Rule, RuleKind, Target, WeightMode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Other,
}

impl Column {
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
        }
    }
}

/// Recognises a header by keywords (lower-case, in order of precedence).
fn classify(header: &str) -> Column {
    let h = header.trim().to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| h.contains(w));
    if has(&[
        "allerg",
        "erityisruokavalio",
        "dietary",
        "ruokarajoit",
        "restriction",
    ]) {
        Column::Allergies
    } else if has(&["plassi", "istumatoive", "seating", "pöytätoive", "pöytäseur", "wish"]) {
        Column::Wish
    } else if has(&["avec", "partner", "seuralainen"]) {
        // Before names: "Avec's full name" is about the avec.
        Column::Avec
    } else if has(&["etu- ja suku", "full name", "koko nimi"]) || h == "nimi" || h == "name" {
        Column::Name
    } else if has(&["etunimi", "first name", "firstname"]) {
        Column::FirstName
    } else if has(&["sukunimi", "last name", "lastname", "surname"]) {
        Column::LastName
    } else if has(&[
        "järjestö",
        "jarjesto",
        "organi",
        "yhdistys",
        "seura",
        "club",
    ]) {
        Column::Organization
    } else if has(&["osallistujaryhmä", "ryhmä", "group", "lippu", "ticket"]) {
        Column::Group
    } else if has(&[
        "juoma", "drink", "alkoholi", "alcohol", "beverage", "viini", "wine",
    ]) {
        Column::Drink
    } else if has(&[
        "ruoka",
        "food",
        "menu",
        "ateria",
        "meal",
        "diet",
        "pääruoka",
        "main course",
    ]) {
        Column::Food
    } else {
        Column::Other
    }
}

pub struct Sheet {
    pub headers: Vec<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

/// Parses tab-, semicolon- or comma-separated text with a header row.
pub fn parse_sheet(text: &str) -> Result<Sheet> {
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    // The most frequent separator in the header row; a single column is tab-separated.
    let delimiter = b"\t;,"
        .iter()
        .copied()
        .map(|d| (first.bytes().filter(|b| *b == d).count(), d))
        .filter(|(count, _)| *count > 0)
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
    let columns: Vec<Column> = headers.iter().map(|h| classify(h)).collect();
    let has_name = columns.contains(&Column::Name)
        || (columns.contains(&Column::FirstName) && columns.contains(&Column::LastName));
    if !has_name {
        bail!(fill(
            "no name column found; expected a header like “Etu- ja sukunimi”, “Nimi” or “Name” (found: {})",
            &[&headers.join(", ")]
        ));
    }
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

pub fn import_tsv(path: &Path) -> Result<Project> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("opening {}", path.display()))?;
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
pub fn merge(project: &mut Project, sheet: &Sheet) -> Result<MergeSummary> {
    let rows = rows(sheet);
    if rows.is_empty() {
        bail!(tr("the sheet has no attendees"));
    }
    let mut summary = MergeSummary::default();
    let mut claimed = HashSet::new();
    let ids: Vec<usize> = rows
        .iter()
        .map(|row| upsert_attendee(project, row, &mut claimed, &mut summary))
        .collect();
    let mut name_map: HashMap<String, Vec<usize>> = HashMap::new();
    for p in &project.attendees {
        name_map.entry(normalize(&p.name)).or_default().push(p.id);
    }
    for (row, &person) in rows.iter().zip(&ids) {
        for (field, text) in [
            (ImportField::Avec, &row.avec),
            (ImportField::Wish, &row.wish),
        ] {
            add_note(project, &name_map, person, field, text.trim(), &mut summary);
        }
    }
    project.validate()?;
    Ok(summary)
}

/// Updates the attendee with the row's name, or adds one; returns their ID.
/// Each existing attendee matches at most one row (`claimed`), so two rows
/// with the same name stay two people.
fn upsert_attendee(
    project: &mut Project,
    row: &Row,
    claimed: &mut HashSet<usize>,
    summary: &mut MergeSummary,
) -> usize {
    let name = normalize(&row.name);
    let same_name = |a: &Attendee| normalize(&a.name) == name;
    if project.attendees.iter().any(|a| same_name(a) && claimed.contains(&a.id)) {
        summary.duplicate_names.push(row.name.clone());
    }
    let existing = project
        .attendees
        .iter()
        .position(|a| same_name(a) && !claimed.contains(&a.id));
    let index = match existing {
        Some(index) => {
            summary.updated += 1;
            index
        }
        None => {
            let id = project
                .attendees
                .iter()
                .map(|a| a.id + 1)
                .max()
                .unwrap_or(0);
            project.attendees.push(Attendee {
                id,
                name: row.name.clone(),
                organization: String::new(),
                participant_group: String::new(),
                tags: Vec::new(),
                food: String::new(),
                drink: String::new(),
                allergies: String::new(),
                extra: BTreeMap::new(),
            });
            summary.added += 1;
            project.attendees.len() - 1
        }
    };
    claimed.insert(project.attendees[index].id);
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
    person: usize,
    field: ImportField,
    text: &str,
    summary: &mut MergeSummary,
) {
    if text.is_empty()
        || project
            .notes
            .iter()
            .any(|n| n.person == person && n.field == field && n.text == text)
    {
        return;
    }
    let exact = name_map
        .get(&normalize(text))
        .and_then(|ids| (ids.len() == 1).then_some(ids[0]))
        .filter(|id| *id != person);
    if let Some(target) = exact {
        project.rules.push(wish_rule(
            project.next_rule_id(),
            person,
            vec![target],
            field == ImportField::Avec,
            text,
        ));
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

/// A standard avec or seating-wish rule.
pub fn wish_rule(id: u64, requester: usize, targets: Vec<usize>, avec: bool, source: &str) -> Rule {
    Rule {
        id,
        label: if avec { "Avec" } else { "Seating wish" }.into(),
        source: source.into(),
        enabled: true,
        kind: RuleKind::Proximity {
            requester,
            target: Target::People {
                ids: targets,
                required: 1,
            },
            costs: if avec {
                ProximityCosts::AVEC
            } else {
                ProximityCosts::WISH
            },
            weight: if avec {
                ProximityCosts::AVEC_WEIGHT
            } else {
                ProximityCosts::WISH_WEIGHT
            },
            weight_mode: if avec {
                WeightMode::Fixed
            } else {
                WeightMode::ShareRequesterBudget
            },
            max_class: None,
        },
    }
}

