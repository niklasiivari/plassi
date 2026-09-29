//! Reading and writing projects as JSON.

use anyhow::Result;

use crate::model::Project;

/// A validated project from JSON bytes.
pub fn parse(bytes: &[u8]) -> Result<Project> {
    let project: Project = serde_json::from_slice(bytes)?;
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
    use anyhow::Context;
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse(&bytes).with_context(|| format!("loading {}", path.display()))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(path: &std::path::Path, project: &Project) -> Result<()> {
    crate::platform::Store::new(&path.to_string_lossy()).write(&encode(project)?)
}
