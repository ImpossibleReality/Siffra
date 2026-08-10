use super::content::{OVERVIEW, OverviewEntry};
use gpui::SharedString;
use siffra::catalog::Catalog;
use std::collections::HashSet;
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Overview,
    Functions,
    Constants,
    Units,
}

impl Tab {
    pub(super) const ALL: [Tab; 4] = [Tab::Overview, Tab::Functions, Tab::Constants, Tab::Units];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Functions => "Functions",
            Self::Constants => "Constants",
            Self::Units => "Units",
        }
    }

    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Functions => "functions",
            Self::Constants => "constants",
            Self::Units => "units",
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Functions => 1,
            Self::Constants => 2,
            Self::Units => 3,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum HelpRow {
    Category {
        tab: Tab,
        name: SharedString,
        count: usize,
        index: usize,
        border_top: bool,
    },
    OverviewEntry(usize),
    Function(usize),
    Constant(usize),
    Unit(usize),
    SearchUnit(usize),
    Gap,
    Empty,
}

pub(super) fn section_key(tab: Tab, category: &str) -> String {
    format!("{}:{category}", tab.key())
}

pub(super) fn overview_entry(index: usize) -> Option<&'static OverviewEntry> {
    OVERVIEW
        .iter()
        .flat_map(|category| category.entries.iter())
        .nth(index)
}

pub(super) fn non_search_rows(
    tab: Tab,
    catalog: &Catalog,
    open_sections: &HashSet<String>,
) -> Vec<HelpRow> {
    let mut rows = Vec::new();
    match tab {
        Tab::Overview => {
            let mut entry_index = 0;
            for (category_index, category) in OVERVIEW.iter().enumerate() {
                rows.push(HelpRow::Category {
                    tab,
                    name: category.name.into(),
                    count: category.entries.len(),
                    index: category_index,
                    border_top: category_index > 0,
                });
                if open_sections.contains(&section_key(tab, category.name)) {
                    rows.extend(
                        (entry_index..entry_index + category.entries.len())
                            .map(HelpRow::OverviewEntry),
                    );
                    rows.push(HelpRow::Gap);
                }
                entry_index += category.entries.len();
            }
        }
        Tab::Functions => {
            for (category_index, (name, indices)) in
                indices_by_category(&catalog.functions, |function| function.category.as_str())
                    .into_iter()
                    .enumerate()
            {
                rows.push(HelpRow::Category {
                    tab,
                    name: SharedString::from(name.to_string()),
                    count: indices.len(),
                    index: category_index,
                    border_top: category_index > 0,
                });
                if open_sections.contains(&section_key(tab, name)) {
                    rows.extend(indices.into_iter().map(HelpRow::Function));
                    rows.push(HelpRow::Gap);
                }
            }
        }
        Tab::Constants => {
            rows.extend((0..catalog.constants.len()).map(HelpRow::Constant));
            rows.push(HelpRow::Gap);
        }
        Tab::Units => {
            for (category_index, (name, indices)) in
                indices_by_category(&catalog.units, |unit| unit.category.as_str())
                    .into_iter()
                    .enumerate()
            {
                rows.push(HelpRow::Category {
                    tab,
                    name: SharedString::from(name.to_string()),
                    count: indices.len(),
                    index: category_index,
                    border_top: category_index > 0,
                });
                if open_sections.contains(&section_key(tab, name)) {
                    rows.extend(indices.into_iter().map(HelpRow::Unit));
                    rows.push(HelpRow::Gap);
                }
            }
        }
    }
    rows
}

pub(super) fn search_rows(tab: Tab, indices: Vec<usize>) -> Vec<HelpRow> {
    if indices.is_empty() {
        return vec![HelpRow::Empty];
    }
    let mut rows: Vec<_> = indices
        .into_iter()
        .map(|index| match tab {
            Tab::Overview => HelpRow::OverviewEntry(index),
            Tab::Functions => HelpRow::Function(index),
            Tab::Constants => HelpRow::Constant(index),
            Tab::Units => HelpRow::SearchUnit(index),
        })
        .collect();
    rows.push(HelpRow::Gap);
    rows
}

pub(super) fn default_open_sections(catalog: &Catalog) -> HashSet<String> {
    OVERVIEW
        .iter()
        .map(|category| section_key(Tab::Overview, category.name))
        .chain(
            catalog
                .functions
                .iter()
                .map(|function| section_key(Tab::Functions, &function.category)),
        )
        .collect()
}

pub(super) fn changed_rows(old: &[HelpRow], new: &[HelpRow]) -> (Range<usize>, usize) {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(old, new)| old == new)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(old, new)| old == new)
        .count();
    (prefix..old.len() - suffix, new.len() - prefix - suffix)
}

fn indices_by_category<'a, T>(
    items: &'a [T],
    category: impl Fn(&'a T) -> &'a str,
) -> Vec<(&'a str, Vec<usize>)> {
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let name = category(item);
        if let Some((_, group)) = groups.iter_mut().find(|(current, _)| *current == name) {
            group.push(index);
        } else {
            groups.push((name, vec![index]));
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use siffra::catalog::catalog;

    #[test]
    fn virtual_rows_preserve_all_open_overview_entries() {
        let catalog = catalog();
        let rows = non_search_rows(Tab::Overview, &catalog, &default_open_sections(&catalog));
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, HelpRow::OverviewEntry(_)))
                .count(),
            17
        );
    }

    #[test]
    fn collapsing_a_section_splices_only_its_contents() {
        let catalog = catalog();
        let open = default_open_sections(&catalog);
        let before = non_search_rows(Tab::Overview, &catalog, &open);
        let mut closed = open;
        closed.remove(&section_key(Tab::Overview, OVERVIEW[0].name));
        let after = non_search_rows(Tab::Overview, &catalog, &closed);

        assert_eq!(changed_rows(&before, &after), (1..6, 0));
    }
}
