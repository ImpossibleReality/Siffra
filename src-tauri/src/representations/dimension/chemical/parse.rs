use crate::representations::dimension::chemical::compound::Compound;
use crate::representations::dimension::chemical::element::Element;
use std::collections::BTreeMap;

fn parse_compound(formula: String) -> Option<Compound> {
    let formula = formula.chars().collect::<Vec<_>>();
    let mut stack = vec![];
    stack.push(BTreeMap::new());
    let mut i = 0;
    while i < formula.len() {
        match formula[i] {
            '(' => {
                i += 1;
                stack.push(BTreeMap::new());
            }
            ')' => {
                let top = stack.pop().unwrap();
                i += 1;
                let i_start = i;
                while i < formula.len() && formula[i].is_ascii_digit() {
                    i += 1;
                }
                let multiplier = formula[i_start..i]
                    .iter()
                    .collect::<String>()
                    .parse::<i32>()
                    .unwrap_or(0)
                    .max(1);
                for (k, v) in top {
                    *stack.last_mut()?.entry(k).or_insert(0) += v * multiplier;
                }
            }
            _ => {
                let i_start = i;
                i += 1;
                while i < formula.len() && formula[i].is_lowercase() {
                    i += 1;
                }
                let name = formula[i_start..i].iter().collect::<String>();
                let i_start = i;
                while i < formula.len() && formula[i].is_ascii_digit() {
                    i += 1;
                }
                let multiplier = formula[i_start..i]
                    .iter()
                    .collect::<String>()
                    .parse::<i32>()
                    .unwrap_or(0)
                    .max(1);
                *stack.last_mut()?.entry(name).or_insert(0) += multiplier;
            }
        }
    }

    let count = stack.pop()?;
    let mut new_count = BTreeMap::new();

    for (k, v) in count {
        *new_count.entry(Element::from_symbol(&k)?).or_insert(0) += v as u16;
    }

    Some(Compound(new_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn parse_compound_returns_correct_compound_for_valid_formula() {
        let formula = "H2O".to_string();
        let compound = parse_compound(formula).unwrap();
        let mut expected = BTreeMap::new();
        expected.insert(Element::from_symbol("H").unwrap(), 2);
        expected.insert(Element::from_symbol("O").unwrap(), 1);
        assert_eq!(compound.0, expected);
    }

    #[test]
    fn parse_compound_returns_correct_compound_for_formula_with_parentheses() {
        let formula = "Ca(OH2)2".to_string();
        let compound = parse_compound(formula).unwrap();
        let mut expected = BTreeMap::new();
        expected.insert(Element::from_symbol("Ca").unwrap(), 1);
        expected.insert(Element::from_symbol("O").unwrap(), 2);
        expected.insert(Element::from_symbol("H").unwrap(), 4);
        assert_eq!(compound.0, expected);
    }

    #[test]
    fn parse_compound_returns_none_for_invalid_formula() {
        let formula = "H2O)2".to_string();
        let compound = parse_compound(formula);
        assert_eq!(compound, None);
    }
}
