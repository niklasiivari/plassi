//! Command-line interface: headless commands for scripts and local agents.

use std::path::Path;

use anyhow::{Context, Result, bail};
use web_time::Duration;

use crate::{import, io, model, print, score, seating, suggest};

const USAGE: &str = "usage: plassi [gui | import INPUT.tsv OUTPUT.json | validate PROJECT.json | \
solve PROJECT.json | bench PROJECT.json [SECONDS] [RUNS] | suggest PROJECT.json | \
export PROJECT.json [OPTION] | export-xlsx PROJECT.json [OPTION] | schema]";

/// Runs a command. Returns `false` if the arguments ask for the GUI instead.
pub fn run(args: &[String]) -> Result<bool> {
    let arg = |i: usize| args.get(i).map(String::as_str);
    let required = |i: usize| arg(i).with_context(|| USAGE.to_string());
    match arg(1) {
        None | Some("gui") => return Ok(false),
        Some("import") => import_command(required(2)?, required(3)?)?,
        Some("validate") => validate(required(2)?)?,
        Some("solve") => solve(required(2)?)?,
        Some("bench") => bench(required(2)?, parse_or(arg(3), 8.0)?, parse_or(arg(4), 2)?)?,
        Some("suggest") => list_suggestions(required(2)?)?,
        Some("export") => export(required(2)?, arg(3).map(str::parse::<usize>).transpose()?)?,
        Some("export-xlsx") => {
            export_xlsx(required(2)?, arg(3).map(str::parse::<usize>).transpose()?)?
        }
        Some("schema") => {
            let schema = schemars::schema_for!(model::Project);
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
        Some(_) => bail!(USAGE),
    }
    Ok(true)
}

fn parse_or<T: std::str::FromStr>(value: Option<&str>, default: T) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    Ok(value.map(str::parse).transpose()?.unwrap_or(default))
}

fn import_command(input: &str, output: &str) -> Result<()> {
    let project = import::import_tsv(Path::new(input))?;
    io::save(Path::new(output), &project)?;
    println!(
        "Imported {} attendees; {} rules; {} source notes",
        project.attendees.len(),
        project.rules.len(),
        project.notes.len()
    );
    Ok(())
}

fn validate(path: &str) -> Result<()> {
    let project = io::load(Path::new(path))?;
    println!(
        "Valid project: {} attendees, {} seats, {} rules, {} pending notes, {} options",
        project.attendees.len(),
        project.layout.seat_count(),
        project.rules.len(),
        project.notes.iter().filter(|note| !note.reviewed).count(),
        project.candidates.len()
    );
    for candidate in &project.candidates {
        let evaluation = score::evaluate(&project, &candidate.assignment);
        println!(
            "{}: wish penalty {}, {} hard violations, {} assignment errors, {} not seated ({})",
            candidate.name,
            evaluation.wish_penalty,
            evaluation.hard_violations,
            evaluation.assignment_errors.len(),
            evaluation.unseated.len(),
            candidate.solver_status.describe()
        );
    }
    Ok(())
}

/// Solves and appends the result to the project as a new option.
fn solve(path: &str) -> Result<()> {
    let mut project = io::load(Path::new(path))?;
    let candidate = seating::solve(&project, None, 0, seating::SOLVE_TIME)?;
    let evaluation = score::evaluate(&project, &candidate.assignment);
    println!(
        "{}: wish penalty {}, {} hard violations",
        candidate.solver_status.describe(),
        evaluation.wish_penalty,
        evaluation.hard_violations
    );
    project.candidates.push(candidate);
    io::save(Path::new(path), &project)
}

/// Runs the search five times and reports each result, for tuning.
fn bench(path: &str, seconds: f64, runs: usize) -> Result<()> {
    if !seconds.is_finite() || seconds <= 0.0 {
        bail!("SECONDS must be a finite number greater than zero");
    }
    if runs == 0 {
        bail!("RUNS must be greater than zero");
    }
    let project = io::load(Path::new(path))?;
    for trial in 1..=bench_trials() {
        let outcome = seating::search(
            &project,
            seating::Options {
                start: None,
                fixed: Default::default(),
                differ_from: None,
                budget: Duration::from_secs_f64(seconds),
                runs,
                seed: Some(trial * 1000),
            },
        )?;
        let evaluation = score::evaluate(&project, &outcome.assignment);
        println!(
            "trial {trial}: score {} (wishes {}, preferences {}, policy {}), {} hard violations, lower bound {}, {} iterations, {}/{} runs at best",
            evaluation.penalty(),
            evaluation.wish_penalty,
            evaluation.preference_penalty,
            evaluation.policy_penalty(),
            evaluation.hard_violations,
            outcome.lower_bound,
            outcome.iterations,
            outcome.runs_at_best,
            outcome.runs
        );
    }
    Ok(())
}

fn list_suggestions(path: &str) -> Result<()> {
    let project = io::load(Path::new(path))?;
    let name = |id: usize| project.person_name(id).to_string();
    for s in suggest::suggestions(&project) {
        let target = match &s.target {
            suggest::SuggestedTarget::People(ids) => ids
                .iter()
                .map(|i| name(*i))
                .collect::<Vec<_>>()
                .join(" or "),
            suggest::SuggestedTarget::Organization(org) => format!("organization {org}"),
        };
        println!(
            "{} → {target} [{}: {}] from “{}”",
            name(s.requester),
            s.confidence.label(),
            s.reason,
            project.notes[s.note].text
        );
    }
    Ok(())
}

/// Writes all prints for option `number` (1-based; default: the latest).
fn export(path: &str, number: Option<usize>) -> Result<()> {
    let project = io::load(Path::new(path))?;
    let option = match number {
        Some(n) => n.saturating_sub(1),
        None => project.candidates.len().saturating_sub(1),
    };
    for kind in print::PrintKind::ALL {
        let out = print::export(&project, Path::new(path), option, kind)?;
        println!("{}: {}", kind.title(), out.display());
    }
    Ok(())
}

/// Writes the Excel handover beside the project (option numbers are 1-based).
fn export_xlsx(path: &str, number: Option<usize>) -> Result<()> {
    let project = io::load(Path::new(path))?;
    let option = match number {
        Some(n) => n.checked_sub(1).context("OPTION must be at least 1")?,
        None => project.candidates.len().saturating_sub(1),
    };
    let bytes = crate::export::workbook(&project, option)?;
    let path = Path::new(path);
    let name = path
        .file_stem()
        .context("project file name")?
        .to_string_lossy();
    let out = path.with_file_name(crate::export::file_name(&project, &name));
    std::fs::write(&out, bytes)?;
    println!("{}", out.display());
    Ok(())
}

/// Trials per bench, from `PLASSI_TRIALS` (default 5).
fn bench_trials() -> u64 {
    std::env::var("PLASSI_TRIALS")
        .ok()
        .and_then(|t| t.parse().ok())
        .unwrap_or(5)
}

#[cfg(test)]
mod tests {
    use super::bench;

    #[test]
    fn bench_rejects_invalid_search_limits_before_loading_a_project() {
        for seconds in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let error = bench("missing.json", seconds, 1).unwrap_err();
            assert!(error.to_string().contains("SECONDS"));
        }
        let error = bench("missing.json", 1.0, 0).unwrap_err();
        assert!(error.to_string().contains("RUNS"));
    }
}
