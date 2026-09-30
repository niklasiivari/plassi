//! Plassi's A4 prints: the template's JSON data in, a PDF out.
//!
//! A crate of its own so the browser build can load it only when printing:
//! Typst is most of the app's size.

use std::sync::OnceLock;

use typst::foundations::{Dict, IntoValue};
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;

static TEMPLATE: &str = include_str!("print.typ");

/// The template's fonts, bundled so prints look the same everywhere
/// (SIL Open Font License, see fonts/).
static FONTS: [&[u8]; 8] = [
    include_bytes!("../fonts/SourceSans3-Regular.otf"),
    include_bytes!("../fonts/SourceSans3-Semibold.otf"),
    include_bytes!("../fonts/SourceSans3-Bold.otf"),
    include_bytes!("../fonts/SourceSans3-It.otf"),
    include_bytes!("../fonts/SourceSerifPro-Regular.otf"),
    include_bytes!("../fonts/SourceSerifPro-Semibold.otf"),
    include_bytes!("../fonts/SourceSerifPro-Bold.otf"),
    include_bytes!("../fonts/SourceSerifPro-It.otf"),
];

fn engine() -> &'static TypstEngine<typst_as_lib::TypstTemplateMainFile> {
    static ENGINE: OnceLock<TypstEngine<typst_as_lib::TypstTemplateMainFile>> = OnceLock::new();
    ENGINE.get_or_init(|| {
        TypstEngine::builder()
            .main_file(TEMPLATE)
            .fonts(FONTS)
            .build()
    })
}

/// Typesets one print. `data` is the JSON the template reads as `data`.
pub fn pdf(data: &str) -> Result<Vec<u8>, String> {
    let mut inputs = Dict::new();
    inputs.insert("data".into(), data.into_value());
    let document: PagedDocument = engine()
        .compile_with_input(inputs)
        .output
        .map_err(|error| format!("typesetting failed: {error:?}"))?;
    typst_pdf::pdf(&document, &Default::default())
        .map_err(|error| format!("PDF export failed: {error:?}"))
}

/// The same, for the browser.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn typeset(data: &str) -> Result<Vec<u8>, String> {
    pdf(data)
}
