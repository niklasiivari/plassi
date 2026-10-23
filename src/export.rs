//! Excel handover for the selected seating: every seat and every guest, plus
//! catering counts. Guest values are always written as text, never formulas.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow, ensure};
use rust_xlsxwriter::{Format, Workbook, Worksheet};

use crate::catalog::Kind;
use crate::model::{Attendee, Project, Side};

struct Labels(bool);

impl Labels {
    fn text<'a>(&self, en: &'a str, fi: &'a str) -> &'a str {
        if self.0 { fi } else { en }
    }
}

pub fn file_name(project: &Project, name: &str) -> String {
    let word = Labels(project.event.print_language.code() == "fi")
        .text("seating and catering", "istumajärjestys ja tarjoilu");
    format!("{name} - {word}.xlsx")
}

/// Snapshot of one option. Incomplete seatings remain useful: empty seats and
/// unseated guests are explicit; malformed assignments fail instead of losing people.
pub fn workbook(project: &Project, candidate: usize) -> Result<Vec<u8>> {
    let option = project
        .candidates
        .get(candidate)
        .ok_or_else(|| anyhow!("no seating option to export"))?;
    let people: HashSet<_> = project.attendees.iter().map(|person| person.id).collect();
    ensure!(
        option.assignment.keys().all(|id| people.contains(id)),
        "seating contains an unknown guest"
    );
    let seats = project.layout.seats();
    let assignment_errors =
        crate::score::assignment_errors_for_seats(project, &option.assignment, &seats);
    ensure!(
        assignment_errors.is_empty(),
        "{}",
        assignment_errors.join("; ")
    );
    let labels = Labels(project.event.print_language.code() == "fi");
    let t = |en, fi| labels.text(en, fi);
    let extra: BTreeSet<&str> = project
        .attendees
        .iter()
        .flat_map(|a| a.extra.keys().map(String::as_str))
        .collect();
    let mut headers: Vec<&str> = vec![
        t("Row", "Rivi"),
        t("Table", "Pöytä"),
        t("Side", "Puoli"),
        t("Seat", "Paikka"),
        t("Name", "Nimi"),
        t("Food", "Ruoka"),
        t("Drinks", "Juomat"),
        t("Allergies / diet", "Allergiat / ruokavalio"),
        t("Organization", "Järjestö"),
        t("Ticket type", "Lipputyyppi"),
        t("Tags", "Tunnisteet"),
        t("Guest ID", "Vieraan tunniste"),
    ];
    headers.extend(extra.iter().copied());
    ensure!(headers.len() <= 16_384, "too many columns for Excel");
    let widths: Vec<f64> = (0..headers.len())
        .map(|i| match i {
            0..=1 => 20.0,
            2..=3 => 10.0,
            4 => 28.0,
            7 => 42.0,
            11 => 14.0,
            _ => 26.0,
        })
        .collect();
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    setup(
        sheet,
        t("Seating order", "Istumajärjestys"),
        project,
        &option.name,
        &format!(
            "{}: {} · {}: {} · {}: {}",
            t("Guests", "Vieraat"),
            project.attendees.len(),
            t("Not seated", "Ilman paikkaa"),
            project.unseated_in(&option.assignment).len(),
            t("Empty seats", "Tyhjät paikat"),
            project
                .layout
                .seat_count()
                .saturating_sub(option.assignment.len())
        ),
        &headers,
        &widths,
    )?;
    sheet.set_freeze_panes(5, 5)?;
    let occupant: HashMap<&str, &Attendee> = project
        .attendees
        .iter()
        .filter_map(|a| option.assignment.get(&a.id).map(|s| (s.as_str(), a)))
        .collect();
    let mut row = 5;
    let mut groups: Vec<(String, Vec<&Attendee>)> = Vec::new();
    for room_row in &project.layout.rows {
        for table in &room_row.tables {
            let name = crate::print::table_name(project, &table.id);
            let mut people = Vec::new();
            for seat in seats.iter().filter(|s| s.table_id == table.id) {
                let person = occupant.get(seat.id.as_str()).copied();
                let mut values = vec![
                    room_row.label.clone(),
                    name.clone(),
                    match seat.side {
                        Side::Left => t("Left", "Vasen"),
                        Side::Right => t("Right", "Oikea"),
                    }
                    .into(),
                    (seat.index + 1).to_string(),
                ];
                values.extend(person_values(project, person, &extra));
                if person.is_none() {
                    values[4] = t("Empty seat", "Tyhjä paikka").into();
                }
                write_row(
                    sheet,
                    row,
                    &values,
                    &widths,
                    groups.len() % 2 == 1,
                    if person.is_some_and(|a| !crate::print::allergy_text(&a.allergies).is_empty())
                    {
                        &[7]
                    } else {
                        &[]
                    },
                )?;
                if let Some(a) = person {
                    people.push(a);
                }
                row += 1;
            }
            groups.push((format!("{} · {name}", groups.len() + 1), people));
        }
    }
    let unseated: Vec<_> = project
        .attendees
        .iter()
        .filter(|a| !option.assignment.contains_key(&a.id))
        .collect();
    for a in &unseated {
        let mut values = vec![
            String::new(),
            t("Not seated", "Ilman paikkaa").into(),
            String::new(),
            String::new(),
        ];
        values.extend(person_values(project, Some(a), &extra));
        let highlights: &[usize] = if crate::print::allergy_text(&a.allergies).is_empty() {
            &[1]
        } else {
            &[1, 7]
        };
        write_row(sheet, row, &values, &widths, false, highlights)?;
        row += 1;
    }
    finish(sheet, row, headers.len() as u16)?;
    groups.push((t("Not seated", "Ilman paikkaa").into(), unseated));
    groups.insert(
        0,
        (
            t("All guests", "Kaikki vieraat").into(),
            project.attendees.iter().collect(),
        ),
    );

    let summary = book.add_worksheet();
    let widths = [30.0, 24.0, 45.0, 12.0];
    setup(
        summary,
        t("Catering totals", "Tarjoilumäärät"),
        project,
        &option.name,
        t(
            "All guests includes unseated guests. Multiple choices count once per selected category. Blank choices are shown as Not specified.",
            "Kaikki vieraat sisältää myös ilman paikkaa olevat. Useat valinnat lasketaan valituittain. Tyhjät valinnat näkyvät muodossa Ei ilmoitettu.",
        ),
        &[
            t("Table / scope", "Pöytä / ryhmä"),
            t("Type", "Tyyppi"),
            t("Choice", "Valinta"),
            t("Count", "Määrä"),
        ],
        &widths,
    )?;
    let mut row = 5;
    for (index, (scope, people)) in groups.iter().enumerate() {
        let mut counts = vec![(
            t("Guests", "Vieraat").to_string(),
            String::new(),
            people.len(),
        )];
        for (kind, label) in [
            (Kind::Food, t("Food", "Ruoka")),
            (Kind::Drink, t("Drinks", "Juomat")),
        ] {
            let mut choices = BTreeMap::<String, usize>::new();
            for a in people {
                let mut names = project.category_names(kind, a);
                if names.is_empty() {
                    let raw = kind.raw(a).trim();
                    names.push(if raw.is_empty() {
                        t("Not specified", "Ei ilmoitettu").into()
                    } else {
                        raw.into()
                    });
                }
                for name in names {
                    *choices.entry(name).or_default() += 1;
                }
            }
            counts.extend(
                choices
                    .into_iter()
                    .map(|(choice, count)| (label.to_string(), choice, count)),
            );
        }
        for (kind, choice, count) in counts {
            write_row(
                summary,
                row,
                &[scope.clone(), kind, choice, String::new()],
                &widths,
                index % 2 == 1,
                &[],
            )?;
            summary.write_number_with_format(row, 3, count as f64, &body(index % 2 == 1))?;
            row += 1;
        }
    }
    finish(summary, row, 4)?;
    Ok(book.save_to_buffer()?)
}

fn person_values(
    project: &Project,
    person: Option<&Attendee>,
    extra: &BTreeSet<&str>,
) -> Vec<String> {
    let Some(a) = person else {
        return vec![String::new(); 8 + extra.len()];
    };
    let mut values = vec![
        a.name.clone(),
        project.category_name(Kind::Food, a),
        project.category_name(Kind::Drink, a),
        a.allergies.clone(),
        project.category_name(Kind::Organization, a),
        project.category_name(Kind::Ticket, a),
        a.tags.join(", "),
        a.id.to_string(),
    ];
    values.extend(
        extra
            .iter()
            .map(|key| a.extra.get(*key).cloned().unwrap_or_default()),
    );
    values
}

fn body(band: bool) -> Format {
    let format = Format::new()
        .set_font_name("Calibri")
        .set_font_size(11)
        .set_text_wrap()
        .set_align(rust_xlsxwriter::FormatAlign::Top);
    if band {
        format.set_background_color("F0F5F5")
    } else {
        format
    }
}

fn setup(
    sheet: &mut Worksheet,
    name: &str,
    project: &Project,
    option: &str,
    note: &str,
    headers: &[&str],
    widths: &[f64],
) -> Result<()> {
    sheet
        .set_name(name)?
        .set_screen_gridlines(false)
        .set_landscape()
        .set_paper_size(9);
    sheet.set_print_fit_to_pages(1, 0);
    let last = (headers.len() - 1) as u16;
    let title = if project.event.title.trim().is_empty() {
        name
    } else {
        &project.event.title
    };
    sheet.merge_range(
        0,
        0,
        0,
        last,
        title,
        &Format::new()
            .set_bold()
            .set_font_size(20)
            .set_font_color("234E52"),
    )?;
    sheet.merge_range(
        1,
        0,
        1,
        last,
        &format!("{} · {option}", project.event.subtitle),
        &Format::new().set_font_size(12),
    )?;
    sheet.merge_range(
        2,
        0,
        2,
        last,
        note,
        &Format::new().set_text_wrap().set_font_size(11),
    )?;
    sheet
        .set_row_height(0, 32)?
        .set_row_height(1, 24)?
        .set_row_height(2, 42)?
        .set_row_height(4, 30)?;
    let header = Format::new()
        .set_bold()
        .set_font_color("FFFFFF")
        .set_background_color("234E52")
        .set_text_wrap();
    for (col, text) in headers.iter().enumerate() {
        sheet.set_column_width(col as u16, widths[col])?;
        sheet.write_string_with_format(4, col as u16, *text, &header)?;
    }
    sheet.set_freeze_panes(5, 0)?.set_repeat_rows(0, 4)?;
    Ok(())
}

fn write_row(
    sheet: &mut Worksheet,
    row: u32,
    values: &[String],
    widths: &[f64],
    band: bool,
    highlight: &[usize],
) -> Result<()> {
    let format = body(band);
    let marked = format
        .clone()
        .set_background_color("FFF0CC")
        .set_font_color("784400")
        .set_bold();
    let mut lines = 1;
    for (col, text) in values.iter().enumerate() {
        // Allow for wrapping and explicit newlines; never silently truncate a long answer.
        lines = lines.max(
            text.split('\n')
                .map(|line| {
                    ((line.chars().count() as f64 / (widths[col] * 0.85)).ceil() as u32).max(1)
                })
                .sum(),
        );
        sheet.write_string_with_format(
            row,
            col as u16,
            text,
            if highlight.contains(&col) {
                &marked
            } else {
                &format
            },
        )?;
    }
    sheet.set_row_height(row, (lines as f64 * 16.0 + 8.0).min(409.0))?;
    Ok(())
}

fn finish(sheet: &mut Worksheet, next: u32, columns: u16) -> Result<()> {
    if next > 5 {
        sheet.autofilter(4, 0, next - 1, columns - 1)?;
    }
    sheet.set_print_area(0, 0, next.saturating_sub(1).max(4), columns - 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Layout, PrintLanguage};
    use calamine::{DataType, Reader, Xlsx};
    use std::io::Cursor;

    fn fixture() -> Project {
        let mut p = crate::example::project();
        p.attendees.truncate(3);
        p.rules.clear();
        p.notes.clear();
        p.event.print_language = PrintLanguage::English;
        p.layout = Layout::simple(1, 2);
        p.attendees[0].name = "=SUM(1,2)".into();
        p.attendees[0].allergies = "Nuts\nAlso avoid traces".into();
        p.attendees[0]
            .extra
            .insert("Arrival note".into(), "Late arrival".into());
        p.attendees[1].food.clear();
        p.attendees[2].food = "plant based".into();
        p.catalog.foods = vec![crate::catalog::Category {
            name: "Vegan".into(),
            aliases: vec!["plant based".into()],
            ..Default::default()
        }];
        let mut option = p.empty_option("Service option".into());
        option.assignment = [(0, "table-1:right:1".into()), (1, "table-1:left:0".into())].into();
        p.candidates = vec![option];
        p
    }

    #[test]
    fn workbook_preserves_seat_order_guest_data_and_catering_counts() {
        let p = fixture();
        let mut xlsx = Xlsx::new(Cursor::new(workbook(&p, 0).unwrap())).unwrap();
        assert_eq!(xlsx.sheet_names(), ["Seating order", "Catering totals"]);
        let seats = xlsx.worksheet_range("Seating order").unwrap();
        let cell = |r, c| seats.get_value((r, c)).unwrap().to_string();
        assert_eq!(cell(5, 4), "Guest 02");
        assert_eq!(cell(6, 4), "Empty seat");
        assert_eq!(cell(7, 4), "Empty seat");
        assert_eq!(cell(8, 4), "=SUM(1,2)");
        assert_eq!(cell(8, 2), "Right");
        assert_eq!(cell(8, 3), "2");
        assert_eq!(cell(8, 7), "Nuts\nAlso avoid traces");
        assert_eq!(cell(8, 12), "Late arrival");
        assert_eq!(cell(9, 1), "Not seated");
        assert_eq!(cell(9, 5), "Vegan");
        assert!(
            xlsx.worksheet_formula("Seating order")
                .unwrap()
                .used_cells()
                .all(|(_, _, f)| f.is_empty())
        );
        let totals = xlsx.worksheet_range("Catering totals").unwrap();
        let count = |scope: &str, kind: &str, choice: &str| {
            totals
                .rows()
                .find(|r| {
                    r[0] == scope && r[1] == kind && r[2].get_string().unwrap_or_default() == choice
                })
                .unwrap()[3]
                .to_string()
        };
        assert_eq!(count("All guests", "Guests", ""), "3");
        assert_eq!(count("All guests", "Food", "Not specified"), "1");
        assert_eq!(count("All guests", "Food", "Vegan"), "1");
        assert_eq!(count("Not seated", "Guests", ""), "1");
        assert_eq!(count("1 · Table 1", "Guests", ""), "2");
    }

    #[test]
    fn exports_selected_option_and_explicit_print_language() {
        let mut p = fixture();
        p.candidates.push(p.empty_option("Unseated option".into()));
        p.event.print_language = PrintLanguage::Finnish;
        let mut xlsx = Xlsx::new(Cursor::new(workbook(&p, 1).unwrap())).unwrap();
        assert_eq!(xlsx.sheet_names(), ["Istumajärjestys", "Tarjoilumäärät"]);
        let seats = xlsx.worksheet_range("Istumajärjestys").unwrap();
        assert!(
            seats
                .get_value((1, 0))
                .unwrap()
                .to_string()
                .contains("Unseated option")
        );
        assert_eq!(seats.get_value((5, 4)).unwrap().to_string(), "Tyhjä paikka");
        assert_eq!(
            seats.get_value((9, 1)).unwrap().to_string(),
            "Ilman paikkaa"
        );
    }

    #[test]
    fn rejects_missing_options_and_invalid_assignments() {
        let mut p = fixture();
        assert!(workbook(&p, 99).is_err());
        p.candidates[0]
            .assignment
            .insert(0, "table-1:left:0".into());
        assert!(workbook(&p, 0).is_err());
        p.candidates[0].assignment.insert(0, "missing-seat".into());
        assert!(workbook(&p, 0).is_err());
    }

    #[test]
    fn multiple_categories_count_each_selection_without_counting_empty_seats() {
        let mut p = fixture();
        p.catalog.drinks = ["Wine", "Water"]
            .map(|name| crate::catalog::Category {
                name: name.into(),
                aliases: vec!["Wine and water".into()],
                ..Default::default()
            })
            .into();
        for a in &mut p.attendees {
            a.drink = "Wine and water".into();
        }
        let mut xlsx = Xlsx::new(Cursor::new(workbook(&p, 0).unwrap())).unwrap();
        let totals = xlsx.worksheet_range("Catering totals").unwrap();
        let drinks: Vec<_> = totals
            .rows()
            .filter(|r| r[0] == "All guests" && r[1] == "Drinks")
            .map(|r| (r[2].to_string(), r[3].to_string()))
            .collect();
        assert_eq!(
            drinks,
            [("Water".into(), "3".into()), ("Wine".into(), "3".into())]
        );
    }
}
