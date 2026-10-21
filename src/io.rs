//! Reading and writing projects as JSON.

#[cfg(not(target_arch = "wasm32"))]
use crate::i18n::fill;
use crate::i18n::tr;
use anyhow::{Context, Result};

use crate::model::Project;

/// A validated project from JSON bytes.
pub fn parse(bytes: &[u8]) -> Result<Project> {
    let project: Project = serde_json::from_slice(bytes).context(tr("Invalid project file"))?;
    project.validate()?;
    Ok(project)
}

/// A validated project as pretty JSON.
pub fn encode(project: &Project) -> Result<Vec<u8>> {
    project.validate()?;
    Ok(serde_json::to_vec_pretty(project)?)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load(path: &std::path::Path) -> Result<Project> {
    let bytes =
        std::fs::read(path).with_context(|| fill("Could not read {}", &[&path.display()]))?;
    parse(&bytes).with_context(|| fill("Could not load {}", &[&path.display()]))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(path: &std::path::Path, project: &Project) -> Result<()> {
    crate::platform::Store::new(&path.to_string_lossy()).write(&encode(project)?)
}
