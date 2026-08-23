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
    pub(super) entry: usize,
    pub(super) matched_content: bool,
}

struct Entry {
    id: u64,
    title: SharedString,
    edited: SharedString,
    content: String,
    lowercase_title: String,
    lowercase_content: String,
    title_mask: u32,
    mask: u32,
}

impl Entry {
    fn row(&self, index: usize, matched_content: bool) -> Row {
        Row {
            id: self.id,
            title: self.title.clone(),
            edited: self.edited.clone(),
            entry: index,
            matched_content,
        }
    }
}

/// The saved calculations prepared for repeated matching: lowercased once when
/// the search opens rather than once per keystroke.
pub(super) struct Haystack {
    entries: Vec<Entry>,
}

impl Haystack {
    pub(super) fn new(documents: Vec<Document>) -> Self {
        let now = now_millis();
        let entries = documents
            .into_iter()
            .map(|document| {
                let title = document.display_title();
                let lowercase_title = title.to_lowercase();
                let lowercase_content = document.content.to_lowercase();
                let title_mask = fuzzy::mask(&lowercase_title);
                Entry {
                    id: document.id,
                    title: SharedString::from(title),
                    edited: SharedString::from(relative_time(document.updated_at, now)),
                    mask: title_mask | fuzzy::mask(&lowercase_content),
                    title_mask,
                    lowercase_title,
                    lowercase_content,
                    content: document.content,
                }
            })
            .collect();

        Self { entries }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn rows(&self, query: &str) -> Vec<Row> {
        if query.is_empty() {
            return self
                .entries
                .iter()
                .enumerate()
                .map(|(index, entry)| entry.row(index, false))
                .collect();
        }

        let query = query.to_lowercase();
        let mask = fuzzy::mask(&query);
        let mut by_name: Vec<(Row, i32)> = Vec::new();
        let mut by_content: Vec<(Row, i32)> = Vec::new();

        for (index, entry) in self.entries.iter().enumerate() {
            if !fuzzy::reachable(mask, entry.mask) {
                continue;
            }

            if fuzzy::reachable(mask, entry.title_mask)
                && let Some(score) = fuzzy::score_lowercase(&query, &entry.lowercase_title)
            {
                by_name.push((entry.row(index, false), score));
                continue;
            }

            let best = entry
                .lowercase_content
                .split('\n')
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .filter_map(|line| fuzzy::score_lowercase(&query, line))
                .max();
            if let Some(best) = best {
                by_content.push((entry.row(index, true), best));
            }
        }

        by_name.sort_by_key(|(_, score)| Reverse(*score));
        by_content.sort_by_key(|(_, score)| Reverse(*score));
        by_name
            .into_iter()
            .chain(by_content)
            .map(|(row, _)| row)
            .collect()
    }

    /// The lines to show under a row, built only for the rows on screen.
    pub(super) fn match_lines(&self, entry: usize, query: &str) -> Vec<MatchLine> {
        let Some(entry) = self.entries.get(entry) else {
            return Vec::new();
        };
        if query.is_empty() {
            return Vec::new();
        }

        let query = query.to_lowercase();
        let mut hits: Vec<(usize, i32)> = entry
            .lowercase_content
            .split('\n')
            .enumerate()
            .map(|(index, line)| (index, line.trim()))
            .filter(|(_, line)| !line.is_empty())
            .filter_map(|(index, line)| {
                fuzzy::score_lowercase(&query, line).map(|score| (index, score))
            })
            .collect();
        hits.sort_by_key(|(_, score)| Reverse(*score));
        hits.truncate(MAX_MATCH_LINES);
        hits.sort_by_key(|(index, _)| *index);

        let lines: Vec<&str> = entry.content.split('\n').collect();
        hits.iter()
            .enumerate()
            .map(|(position, (index, _))| MatchLine {
                text: SharedString::from(lines[*index].trim().to_string()),
                skipped_before: position > 0
                    && lines[hits[position - 1].0 + 1..*index]
                        .iter()
                        .any(|line| !line.trim().is_empty()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(id: u64, title: &str, content: &str) -> Document {
        Document {
            id,
            title: Some(title.to_string()),
            content: content.to_string(),
            cursor: 0,
            created_at: 0,
            updated_at: id as i64,
        }
    }

    fn haystack(documents: Vec<Document>) -> Haystack {
        Haystack::new(documents)
    }

    fn lines(haystack: &Haystack, row: &Row, query: &str) -> Vec<MatchLine> {
        if row.matched_content {
            haystack.match_lines(row.entry, query)
        } else {
            Vec::new()
        }
    }

    #[test]
    fn an_empty_query_lists_everything_in_the_order_it_was_given() {
        let all = haystack(vec![document(2, "Two", ""), document(1, "One", "")]);
        let rows = all.rows("");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [2, 1],
            "the store hands the list over newest first"
        );
        assert!(rows.iter().all(|row| !row.matched_content));
    }

    #[test]
    fn a_name_match_outranks_anything_found_inside_a_calculation() {
        let all = haystack(vec![
            document(1, "Groceries", "12 kg to lb"),
            document(2, "Trip budget", "groceries = 40 USD"),
        ]);
        let rows = all.rows("groceries");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, 1);
        assert!(
            lines(&all, &rows[0], "groceries").is_empty(),
            "a name match shows no lines"
        );
        let found = lines(&all, &rows[1], "groceries");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "groceries = 40 USD");
    }

    #[test]
    fn only_the_best_few_lines_are_shown_and_they_read_in_order() {
        let content = (0..10)
            .map(|index| format!("kg {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let all = haystack(vec![document(1, "Weights", &content)]);
        let rows = all.rows("kg");
        let found = lines(&all, &rows[0], "kg");
        assert_eq!(found.len(), MAX_MATCH_LINES);
        let shown: Vec<_> = found.iter().map(|line| line.text.to_string()).collect();
        let mut sorted = shown.clone();
        sorted.sort();
        assert_eq!(shown, sorted, "lines keep the order they were written in");
    }

    #[test]
    fn a_skipped_line_is_marked_but_a_blank_one_is_not() {
        let all = haystack(vec![
            document(1, "Gaps", "kg 1\n3 + 3\nkg 2"),
            document(2, "Blanks", "kg 1\n\nkg 2"),
        ]);
        let rows = all.rows("kg");

        let gapped = rows.iter().find(|row| row.id == 1).expect("row");
        let gapped = lines(&all, gapped, "kg");
        assert!(!gapped[0].skipped_before);
        assert!(
            gapped[1].skipped_before,
            "a calculation line was left out between the two matches"
        );

        let blanks = rows.iter().find(|row| row.id == 2).expect("row");
        let blanks = lines(&all, blanks, "kg");
        assert!(
            !blanks[1].skipped_before,
            "a blank line hides nothing, so there is nothing to stand in for"
        );
    }

    #[test]
    fn calculations_that_match_nowhere_are_left_out() {
        let all = haystack(vec![document(1, "Trip", "2 + 2")]);
        assert!(all.rows("zzz").is_empty());
    }

    #[test]
    fn matching_lines_keep_the_case_they_were_written_in() {
        let all = haystack(vec![document(1, "Trip", "Budget = 40 USD")]);
        let rows = all.rows("budget");
        assert_eq!(lines(&all, &rows[0], "budget")[0].text, "Budget = 40 USD");
    }

    #[test]
    fn a_document_is_listed_once_however_many_lines_match() {
        let all = haystack(vec![document(1, "Trip", "kg 1\nkg 2\nkg 3")]);
        assert_eq!(all.rows("kg").len(), 1);
    }
}
