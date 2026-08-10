use super::content::OVERVIEW;
use super::rows::Tab;
use crate::fuzzy;
use siffra::catalog::{Catalog, CatalogUnit};
use std::collections::HashSet;

pub(super) struct SearchIndex {
    overview: Vec<Vec<String>>,
    functions: Vec<Vec<String>>,
    constants: Vec<Vec<String>>,
    units: Vec<Vec<String>>,
}

impl SearchIndex {
    pub(super) fn documents(&self, tab: Tab) -> &[Vec<String>] {
        match tab {
            Tab::Overview => &self.overview,
            Tab::Functions => &self.functions,
            Tab::Constants => &self.constants,
            Tab::Units => &self.units,
        }
    }
}

pub(super) fn build_search_index(
    catalog: &Catalog,
    searchable_units: &[CatalogUnit],
) -> SearchIndex {
    fn normalized(keys: impl IntoIterator<Item = String>) -> Vec<String> {
        keys.into_iter().map(|key| key.to_lowercase()).collect()
    }

    SearchIndex {
        overview: OVERVIEW
            .iter()
            .flat_map(|category| category.entries)
            .map(|entry| {
                normalized(
                    [entry.title.to_string(), entry.description.to_string()]
                        .into_iter()
                        .chain(entry.keywords.iter().map(|keyword| (*keyword).to_string()))
                        .chain(
                            entry
                                .examples
                                .iter()
                                .map(|example| example.input.to_string()),
                        ),
                )
            })
            .collect(),
        functions: catalog
            .functions
            .iter()
            .map(|function| {
                normalized([
                    function.name.clone(),
                    function.signature.clone(),
                    function.description.clone(),
                ])
            })
            .collect(),
        constants: catalog
            .constants
            .iter()
            .map(|constant| normalized([constant.name.clone(), constant.description.clone()]))
            .collect(),
        units: searchable_units
            .iter()
            .map(|unit| {
                normalized(
                    [unit.symbol.clone(), unit.name.clone()]
                        .into_iter()
                        .chain(unit.aliases.iter().cloned()),
                )
            })
            .collect(),
    }
}

pub(super) fn scored_document_indices(query: &str, documents: &[Vec<String>]) -> Vec<usize> {
    let query = query.to_lowercase();
    let mut scored: Vec<(usize, i32)> = documents
        .iter()
        .enumerate()
        .filter_map(|(order, keys)| {
            let score = keys
                .iter()
                .filter_map(|key| fuzzy::score_lowercase(&query, key))
                .max()?;
            Some((order, score))
        })
        .collect();
    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.into_iter().map(|(index, _)| index).collect()
}

pub(super) fn searchable_units(catalog: &Catalog) -> Vec<CatalogUnit> {
    let named: HashSet<&str> = catalog
        .units
        .iter()
        .map(|unit| unit.symbol.as_str())
        .collect();
    let mut expanded = Vec::new();

    for unit in &catalog.units {
        expanded.push(CatalogUnit {
            category: unit.category.clone(),
            name: unit.name.clone(),
            symbol: unit.symbol.clone(),
            aliases: unit.aliases.clone(),
            si_prefixes: unit.si_prefixes,
        });
        if !unit.si_prefixes {
            continue;
        }

        for prefix in &catalog.prefixes {
            let symbol = format!("{}{}", prefix.symbol, unit.symbol);
            if named.contains(symbol.as_str()) {
                continue;
            }
            expanded.push(CatalogUnit {
                category: unit.category.clone(),
                name: format!("{}{}", prefix.name, unit.name),
                symbol,
                aliases: unit
                    .aliases
                    .iter()
                    .filter(|alias| {
                        alias.len() >= 3
                            && alias
                                .chars()
                                .all(|character| character.is_ascii_lowercase())
                    })
                    .map(|alias| format!("{}{}", prefix.name, alias))
                    .collect(),
                si_prefixes: false,
            });
        }
    }

    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    use siffra::catalog::catalog;

    #[test]
    fn prefix_expansion_makes_spelled_and_symbol_forms_searchable() {
        let catalog = catalog();
        let units = searchable_units(&catalog);
        assert!(units.iter().any(|unit| unit.symbol == "nF"));
        assert!(units.iter().any(|unit| unit.name == "nanofarad"));
    }

    #[test]
    fn background_search_indices_keep_catalog_identity() {
        let catalog = catalog();
        let units = searchable_units(&catalog);
        let index = build_search_index(&catalog, &units);

        let sqrt = catalog
            .functions
            .iter()
            .position(|function| function.name == "sqrt")
            .unwrap();
        assert_eq!(
            scored_document_indices("sqrt", index.documents(Tab::Functions))[0],
            sqrt
        );

        let nanofarad = units.iter().position(|unit| unit.symbol == "nF").unwrap();
        assert_eq!(
            scored_document_indices("nF", index.documents(Tab::Units))[0],
            nanofarad
        );
    }
}
