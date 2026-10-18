//! Turn uploaded text files or workbooks into independently mapped sheets.

use std::io::Cursor;

use anyhow::{Result, bail};
use calamine::Reader;

use crate::i18n::tr;

/// Each worksheet's name and CSV text, ready for the same preview as pasted rows.
pub fn read_sheets(name: &str, bytes: &[u8]) -> Result<Vec<(String, String)>> {
    let extension = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "csv" | "tsv" | "txt" => Ok(vec![(
            name.to_string(),
            String::from_utf8_lossy(bytes)
                .trim_start_matches('\u{feff}')
                .to_string(),
        )]),
        "xlsx" | "xls" | "xlsm" | "xlsb" | "ods" => {
            let mut workbook = calamine::open_workbook_auto_from_rs(Cursor::new(bytes))?;
            let mut sheets = Vec::new();
            for name in workbook.sheet_names() {
                let range = workbook.worksheet_range(&name)?;
                if range.is_empty() {
                    continue;
                }
                let mut writer = csv::Writer::from_writer(Vec::new());
                for row in range.rows() {
                    writer.write_record(row.iter().map(ToString::to_string))?;
                }
                sheets.push((name, String::from_utf8(writer.into_inner()?)?));
            }
            if sheets.is_empty() {
                bail!(tr("No non-empty worksheets in this file"));
            }
            Ok(sheets)
        }
        _ => bail!(tr("Choose an Excel, CSV, TSV or ODS file")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excel_worksheets_have_independent_headers_and_empty_tabs_are_skipped() {
        let sheets = read_sheets(
            "import.xlsx",
            include_bytes!("../../tests/fixtures/import.xlsx"),
        )
        .unwrap();
        assert_eq!(sheets.len(), 2);
        assert_eq!(sheets[0].0, "Sheet 1");
        let first = crate::import::parse_sheet(&sheets[0].1).unwrap();
        assert_eq!(first.headers, ["Full name", "Club"]);
        assert_eq!(first.rows, [vec!["X", "ABC"], vec!["Y", "XYZ"]]);
        let second = crate::import::parse_sheet(&sheets[1].1).unwrap();
        assert_eq!(second.headers, ["Attendee", "Meal"]);
        assert_eq!(second.rows, [vec!["Z", "Vegetarian"]]);
    }

    #[test]
    fn text_files_keep_bom_and_quoted_headers_out_of_the_mapping() {
        let sheets = read_sheets(
            "import.CSV",
            "\u{feff}\"Person; name; field\",Organization\nX,ABC".as_bytes(),
        )
        .unwrap();
        let sheet = crate::import::parse_sheet(&sheets[0].1).unwrap();
        assert_eq!(sheet.headers, ["Person; name; field", "Organization"]);
        assert_eq!(sheet.rows, [vec!["X", "ABC"]]);
    }

    #[test]
    fn invalid_workbooks_and_unsupported_files_are_rejected() {
        assert!(read_sheets("import.xlsx", b"not a workbook").is_err());
        assert!(read_sheets("import.pdf", b"not a sheet").is_err());
    }
}
