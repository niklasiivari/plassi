//! Seating plans made from editable worksheet cells, grouped by room row.

use super::Labels;
use crate::catalog::Kind;
use crate::model::{Attendee, Project, Side, TableShape, TableSpec};
use anyhow::{Result, ensure};
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Worksheet};
use std::collections::HashMap;

const TABLE_COLUMNS: u16 = 6;
const TABLES_ACROSS: usize = 2;

struct SeatCell {
    row: u32,
    side: i8,
    number: String,
    guest: String,
    allergy: bool,
    empty: bool,
}

/// Use each seat's local geometry so uneven round tables keep their actual
/// clockwise order instead of incorrectly pairing left and right indexes.
fn seat_cells(
    project: &Project,
    table: &TableSpec,
    occupants: &HashMap<&str, &Attendee>,
    labels: &Labels,
    catering: bool,
) -> Vec<SeatCell> {
    let mut positions = Vec::new();
    for (side, count, suffix) in [
        (Side::Left, table.left, "left"),
        (Side::Right, table.right, "right"),
    ] {
        for index in 0..count {
            let (x, y, _) = table.local_seat(side, index);
            positions.push((side, index, suffix, x, y));
        }
    }
    let mut levels: Vec<_> = positions.iter().map(|p| p.4).collect();
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    positions
        .into_iter()
        .map(|(side, index, suffix, x, y)| {
            let person = occupants
                .get(format!("{}:{suffix}:{index}", table.id).as_str())
                .copied();
            let number = format!(
                "{}{}",
                match side {
                    Side::Left => labels.text("L", "V"),
                    Side::Right => labels.text("R", "O"),
                },
                index + 1
            );
            SeatCell {
                row: levels
                    .iter()
                    .position(|level| (*level - y).abs() < 0.01)
                    .unwrap() as u32,
                side: if x.abs() < 0.01 {
                    0
                } else if x < 0.0 {
                    -1
                } else {
                    1
                },
                number,
                guest: guest_text(project, person, labels, catering),
                allergy: person
                    .is_some_and(|a| !crate::print::allergy_text(&a.allergies).is_empty()),
                empty: person.is_none(),
            }
        })
        .collect()
}

fn guest_text(
    project: &Project,
    person: Option<&Attendee>,
    labels: &Labels,
    catering: bool,
) -> String {
    let Some(person) = person else {
        return labels.text("Empty seat", "Tyhjä paikka").into();
    };
    let mut text = person.name.clone();
    if !catering {
        return text;
    }
    for (label, value) in [
        (
            labels.text("Food", "Ruoka"),
            project.category_name(Kind::Food, person),
        ),
        (
            labels.text("Drinks", "Juomat"),
            project.category_name(Kind::Drink, person),
        ),
        (
            labels.text("Allergies", "Allergiat"),
            crate::print::allergy_text(&person.allergies).to_string(),
        ),
    ] {
        if !value.trim().is_empty() {
            text.push_str(&format!("\n{label}: {value}"));
        }
    }
    text
}

pub(super) fn write(
    sheet: &mut Worksheet,
    project: &Project,
    candidate: usize,
    labels: &Labels,
    catering: bool,
) -> Result<()> {
    let option = &project.candidates[candidate];
    let occupants: HashMap<_, _> = project
        .attendees
        .iter()
        .filter_map(|a| option.assignment.get(&a.id).map(|s| (s.as_str(), a)))
        .collect();
    let across = project
        .layout
        .rows
        .iter()
        .map(|r| r.tables.len())
        .max()
        .unwrap_or(1)
        .clamp(1, TABLES_ACROSS);
    let last = across as u16 * TABLE_COLUMNS - 2;
    sheet
        .set_name(if catering {
            labels.text("Catering layout", "Tarjoilukartta")
        } else {
            labels.text("Table layout", "Pöytäkartta")
        })?
        .set_screen_gridlines(false)
        .set_landscape()
        .set_paper_size(8);
    for block in 0..across as u16 {
        for (offset, width) in [30.0, 5.0, 11.0, 5.0, 30.0, 4.0].into_iter().enumerate() {
            sheet.set_column_width(block * TABLE_COLUMNS + offset as u16, width)?;
        }
    }
    let accent = Format::new().set_bold().set_font_color("234E52");
    let title = if project.event.title.trim().is_empty() {
        labels.text("Table layout", "Pöytäkartta")
    } else {
        &project.event.title
    };
    sheet.merge_range(0, 0, 0, last, title, &accent.clone().set_font_size(20))?;
    sheet.set_row_height(0, 32)?;
    sheet.merge_range(
        1,
        0,
        1,
        last,
        &option.name,
        &Format::new().set_font_size(12),
    )?;
    sheet.set_row_height(1, 24)?;
    sheet.merge_range(
        2,
        0,
        2,
        last,
        &format!(
            "{}: {} · {}",
            labels.text("Not seated", "Ilman paikkaa"),
            project.unseated_in(&option.assignment).len(),
            labels.text("Tables shown upright", "Pöydät esitetään pystysuunnassa")
        ),
        &Format::new(),
    )?;
    sheet.set_freeze_panes(4, 0)?.set_repeat_rows(0, 2)?;
    let heading = Format::new()
        .set_bold()
        .set_font_color("FFFFFF")
        .set_background_color("234E52")
        .set_align(FormatAlign::Center)
        .set_text_wrap();
    let guest = Format::new()
        .set_font_name("Calibri")
        .set_font_size(11)
        .set_text_wrap()
        .set_align(FormatAlign::VerticalCenter)
        .set_border(FormatBorder::Thin)
        .set_border_color("B8CBCB");
    let number = guest
        .clone()
        .set_bold()
        .set_align(FormatAlign::Center)
        .set_background_color("E4EFEF");
    let table_format = accent
        .clone()
        .set_background_color("C6DCDC")
        .set_align(FormatAlign::Center)
        .set_align(FormatAlign::VerticalCenter)
        .set_text_wrap()
        .set_border(FormatBorder::Medium)
        .set_border_color("234E52");
    let mut row = 4;
    for room_row in &project.layout.rows {
        for tables in room_row.tables.chunks(TABLES_ACROSS) {
            sheet.merge_range(row, 0, row, last, &room_row.label, &accent)?;
            sheet.set_row_height(row, 24)?;
            row += 1;
            let mut heights = Vec::<f64>::new();
            for (block, table) in tables.iter().enumerate() {
                let col = block as u16 * TABLE_COLUMNS;
                let name = crate::print::table_name(project, &table.id);
                let title = if table.shape == TableShape::Round {
                    format!("{name} · {}", labels.text("Round", "Pyöreä"))
                } else {
                    name.clone()
                };
                sheet.merge_range(row, col, row, col + 4, &title, &heading)?;
                let cells = seat_cells(project, table, &occupants, labels, catering);
                let count = cells.iter().map(|c| c.row + 1).max().unwrap_or(1);
                ensure!(
                    row + count + 4 < 1_048_576,
                    "table layout exceeds Excel's row limit"
                );
                heights.resize(
                    heights.len().max(count as usize),
                    if catering { 36.0 } else { 28.0 },
                );
                let start = row + 1;
                // Top/bottom seats of odd round tables span the full table width.
                // The remaining middle cells form the tabletop between guests.
                let middle: Vec<_> = (0..count)
                    .filter(|r| !cells.iter().any(|c| c.row == *r && c.side == 0))
                    .collect();
                if let (Some(first), Some(last)) = (middle.first(), middle.last()) {
                    if first == last {
                        sheet.write_string_with_format(
                            start + first,
                            col + 2,
                            &name,
                            &table_format,
                        )?;
                    } else {
                        sheet.merge_range(
                            start + first,
                            col + 2,
                            start + last,
                            col + 2,
                            &name,
                            &table_format,
                        )?;
                    }
                }
                for cell in cells {
                    let r = start + cell.row;
                    let text = if cell.side == 0 {
                        format!("{} · {}", cell.number, cell.guest)
                    } else {
                        cell.guest
                    };
                    let format = if catering && cell.allergy {
                        guest.clone().set_background_color("FFF0CC")
                    } else if cell.empty {
                        guest.clone().set_font_color("687878").set_italic()
                    } else {
                        guest.clone()
                    };
                    if cell.side == 0 {
                        sheet.merge_range(
                            r,
                            col,
                            r,
                            col + 4,
                            &text,
                            &format.clone().set_align(FormatAlign::Center),
                        )?;
                    } else {
                        let (guest_col, seat_col) = if cell.side < 0 {
                            (col, col + 1)
                        } else {
                            (col + 4, col + 3)
                        };
                        sheet.write_string_with_format(r, guest_col, &text, &format)?;
                        sheet.write_string_with_format(r, seat_col, &cell.number, &number)?;
                    }
                    let width = if cell.side == 0 { 75 } else { 26 };
                    let lines: usize = text
                        .split('\n')
                        .map(|line| line.chars().count().div_ceil(width).max(1))
                        .sum();
                    heights[cell.row as usize] =
                        heights[cell.row as usize].max((lines as f64 * 16.0 + 12.0).min(409.0));
                }
            }
            sheet.set_row_height(row, 30)?;
            for (offset, height) in heights.iter().enumerate() {
                sheet.set_row_height(row + 1 + offset as u32, *height)?;
            }
            row += heights.len() as u32 + 3;
        }
    }
    sheet
        .set_print_area(0, 0, row.max(4), last)?
        .set_print_fit_to_pages(1, 0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Layout, PrintLanguage};
    use calamine::{Reader, Xlsx};
    use std::io::Cursor;

    fn fixture() -> Project {
        let mut p = crate::example::project();
        p.event.print_language = PrintLanguage::English;
        p.layout = Layout::grid(2, 3, 2);
        p.attendees.truncate(12);
        p.attendees[0].name = "=SUM(1,2)".into();
        let mut option = p.empty_option("Layout option".into());
        option.assignment = p
            .attendees
            .iter()
            .zip(p.layout.seats())
            .map(|(a, s)| (a.id, s.id))
            .collect();
        option.assignment.remove(&11);
        p.candidates = vec![option];
        p
    }

    #[test]
    fn seating_is_in_cells_with_facing_guests_and_no_drawings() {
        let p = fixture();
        let bytes = crate::export::workbook(&p, 0).unwrap();
        let mut xlsx = Xlsx::new(Cursor::new(&bytes)).unwrap();
        let names = xlsx.worksheet_range("Table layout").unwrap();
        assert_eq!(names.get_value((6, 0)).unwrap().to_string(), "=SUM(1,2)");
        let sheet = xlsx.worksheet_range("Catering layout").unwrap();
        let cell = |r, c| sheet.get_value((r, c)).unwrap().to_string();
        assert!(cell(6, 0).starts_with("=SUM(1,2)\n"));
        assert!(cell(6, 0).contains("Allergies: Nuts"));
        assert_eq!(cell(6, 1), "L1");
        assert_eq!(cell(6, 2), "Table 1");
        assert_eq!(cell(6, 3), "R1");
        assert!(cell(6, 4).starts_with("Guest 04\n"));
        assert_eq!(cell(8, 10), "Empty seat");
        assert!(
            xlsx.worksheet_formula("Table layout")
                .unwrap()
                .used_cells()
                .all(|(_, _, f)| f.is_empty())
        );
        let zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        assert!(
            !zip.file_names()
                .any(|name| name.starts_with("xl/drawings/") || name.starts_with("xl/media/"))
        );
        if let Ok(dir) = std::env::var("PLASSI_EXPORT_PREVIEW_DIR") {
            let mut preview = p;
            preview.attendees[0].name = "Guest 01".into();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                std::path::Path::new(&dir).join("layout.xlsx"),
                crate::export::workbook(&preview, 0).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn round_tables_keep_every_seat_once_even_with_uneven_sides() {
        let mut p = fixture();
        p.candidates[0].assignment.clear();
        for (left, right) in [(3, 3), (2, 5), (1, 0), (0, 0)] {
            let table = &mut p.layout.rows[0].tables[0];
            table.shape = TableShape::Round;
            table.left = left;
            table.right = right;
            let cells = seat_cells(
                &p,
                &p.layout.rows[0].tables[0],
                &HashMap::new(),
                &Labels(false),
                true,
            );
            let positions: std::collections::HashSet<_> =
                cells.iter().map(|c| (c.row, c.side)).collect();
            assert_eq!(positions.len(), left + right);
            assert!(crate::export::workbook(&p, 0).is_ok());
        }
        p.layout = Layout::default();
        assert!(crate::export::workbook(&p, 0).is_ok());
    }
}
