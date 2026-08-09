use super::relative_time::relative_time;
use crate::documents::{Document, now_millis};
use crate::fuzzy;
use gpui::SharedString;
use std::cmp::Reverse;

pub(super) const MAX_MATCH_LINES: usize = 4;

pub(super) struct MatchLine {
    pub(super) text: SharedString,
    /// Whether non-empty lines were skipped before this match.
    pub(super) skipped_before: bool,
}

pub(super) struct Row {
    pub(super) id: u64,
    pub(super) title: SharedString,
    pub(super) edited: SharedString,
    pub(super) lines: Vec<MatchLine>,
}

pub(super) fn rows_for(query: &str, all: &[Document]) -> Vec<Row> {
    let now = now_millis();

    if query.is_empty() {
        return all
            .iter()
            .map(|document| row_at(document, Vec::new(), now))
            .collect();
    }

    let mut by_name: Vec<(Row, i32)> = Vec::new();
    let mut by_content: Vec<(Row, i32)> = Vec::new();

    for document in all {
        if let Some(score) = fuzzy::score(query, &document.display_title()) {
            by_name.push((row_at(document, Vec::new(), now), score));
            continue;
        }

        let lines: Vec<&str> = document.content.split('\n').collect();
        let mut hits: Vec<(usize, &str, i32)> = lines
            .iter()
            .enumerate()
            .map(|(index, line)| (index, line.trim()))
            .filter(|(_, line)| !line.is_empty())
            .filter_map(|(index, line)| fuzzy::score(query, line).map(|score| (index, line, score)))
            .collect();
        hits.sort_by_key(|(_, _, score)| Reverse(*score));
        hits.truncate(MAX_MATCH_LINES);
        if hits.is_empty() {
            continue;
        }

        let best = hits[0].2;
        hits.sort_by_key(|(index, _, _)| *index);
        let matches = hits
            .iter()
            .enumerate()
            .map(|(position, (index, text, _))| MatchLine {
                text: SharedString::from((*text).to_string()),
                skipped_before: position > 0
                    && lines[hits[position - 1].0 + 1..*index]
                        .iter()
                        .any(|line| !line.trim().is_empty()),
            })
            .collect();
        by_content.push((row_at(document, matches, now), best));
    }

    by_name.sort_by_key(|(_, score)| Reverse(*score));
    by_content.sort_by_key(|(_, score)| Reverse(*score));
    by_name
        .into_iter()
        .chain(by_content)
        .map(|(row, _)| row)
        .collect()
}

fn row_at(document: &Document, lines: Vec<MatchLine>, now: i64) -> Row {
    Row {
        id: document.id,
        title: SharedString::from(document.display_title()),
        edited: SharedString::from(relative_time(document.updated_at, now)),
        lines,
    }
}
