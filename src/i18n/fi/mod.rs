//! Finnish text, one catalog per part of the app: (English, Finnish) pairs.

mod chart;
mod common;
mod graph;
mod pages;
mod people;

pub(super) fn catalog() -> impl Iterator<Item = (&'static str, &'static str)> {
    [common::STRINGS, chart::STRINGS, graph::STRINGS, people::STRINGS, pages::STRINGS]
        .into_iter()
        .flatten()
        .copied()
}
