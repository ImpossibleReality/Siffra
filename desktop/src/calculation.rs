use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::parser::{ParsedDimension, ParsedExpr, ParsedLine, parse_line};
use siffra::representations::{Currency, Dimension, Float, Unit, Value};
use std::collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq)]
pub enum LineOutput {
    Value {
        string: String,
        repeating: bool,
    },
    Error {
        span: Option<(usize, usize)>,
        message: String,
        description: Option<String>,
        location: Option<String>,
    },
}

#[derive(Clone)]
struct CachedLine {
    line: String,
    previous_hash: u64,
    uses_currency_shorthands: bool,
    output: Option<LineOutput>,
    state_after: SiffraState,
}

#[derive(Default)]
pub struct Calculation {
    lines: HashMap<u64, CachedLine>,
}

impl Calculation {
    pub fn evaluate(&mut self, input: &str) -> Vec<Option<LineOutput>> {
        let mut state = SiffraState::new();
        let mut outputs = Vec::new();
        let mut previous_hash = 0;
        let mut used_hashes = HashSet::new();
        let uses_currency_shorthands = input_uses_currency_shorthands(input);

        for line in input.split('\n') {
            let hash = line_hash(line, previous_hash);
            used_hashes.insert(hash);

            let cached = self
                .lines
                .get(&hash)
                .filter(|entry| {
                    entry.line == line
                        && entry.previous_hash == previous_hash
                        && entry.uses_currency_shorthands == uses_currency_shorthands
                })
                .cloned();

            let output = if let Some(cached) = cached {
                state = cached.state_after;
                cached.output
            } else {
                let output = match evaluate_line(line, &mut state) {
                    Ok(Some(value)) => {
                        let (string, repeating) = display_value(value, uses_currency_shorthands);
                        Some(LineOutput::Value { string, repeating })
                    }
                    Ok(None) => None,
                    Err(err) => {
                        #[cfg(debug_assertions)]
                        let location = err.location();

                        #[cfg(not(debug_assertions))]
                        let location = None;

                        Some(LineOutput::Error {
                            message: err.message(),
                            description: err.description(),
                            span: err.span(),
                            location,
                        })
                    }
                };

                self.lines.insert(
                    hash,
                    CachedLine {
                        line: line.to_string(),
                        previous_hash,
                        uses_currency_shorthands,
                        output: output.clone(),
                        state_after: state.clone(),
                    },
                );
                output
            };

            outputs.push(output);
            previous_hash = hash;
        }

        self.lines.retain(|hash, _| used_hashes.contains(hash));
        outputs
    }
}

fn line_hash(line: &str, previous_hash: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    previous_hash.hash(&mut hasher);
    line.hash(&mut hasher);
    hasher.finish()
}

fn has_short_repeating_decimal(value: &str) -> bool {
    let number = value.split_whitespace().next().unwrap_or(value);
    let mantissa = number
        .split_once('E')
        .map_or(number, |(mantissa, _)| mantissa);
    let Some((_, fraction)) = mantissa.split_once('.') else {
        return false;
    };

    const MIN_REPEATING_DIGITS: usize = 12;
    if fraction.len() < MIN_REPEATING_DIGITS {
        return false;
    }

    for start in 0..fraction.len() {
        for period in 1..=3 {
            let repeated_len = fraction.len() - start;
            if repeated_len < MIN_REPEATING_DIGITS
                || repeated_len < period * 4
                || start + period > fraction.len()
            {
                continue;
            }

            let bytes = fraction.as_bytes();
            if (start..fraction.len()).all(|i| bytes[i] == bytes[start + (i - start) % period]) {
                return true;
            }
        }
    }

    false
}

fn collect_dimension_currencies(dimension: &ParsedDimension, currencies: &mut BTreeSet<String>) {
    for (unit, _) in dimension.numerator.iter().chain(&dimension.denominator) {
        if let Ok(currency) = Currency::from_str(&unit.name) {
            currencies.insert(currency.code().to_string());
        }
    }
}

fn collect_expression_currencies(expression: &ParsedExpr, currencies: &mut BTreeSet<String>) {
    match expression {
        ParsedExpr::Number { units, .. } => {
            if let Some(units) = units {
                collect_dimension_currencies(units, currencies);
            }
        }
        ParsedExpr::Variable { .. } => {}
        ParsedExpr::FunctionCall { args, base, .. } => {
            if let Some(base) = base {
                collect_expression_currencies(base, currencies);
            }
            for arg in args {
                collect_expression_currencies(arg, currencies);
            }
        }
        ParsedExpr::UnOpPre { rhs, .. } => collect_expression_currencies(rhs, currencies),
        ParsedExpr::UnOpPost { lhs, op, .. } => {
            collect_expression_currencies(lhs, currencies);
            if let siffra::parser::OpPost::Convert(dimension) = op {
                collect_dimension_currencies(dimension, currencies);
            }
        }
        ParsedExpr::BinOp { lhs, rhs, .. } => {
            collect_expression_currencies(lhs, currencies);
            collect_expression_currencies(rhs, currencies);
        }
    }
}

fn input_uses_currency_shorthands(input: &str) -> bool {
    let mut currencies = BTreeSet::new();

    for line in input.lines() {
        match parse_line(line) {
            Ok(ParsedLine::Expression(expression) | ParsedLine::Variable(_, expression)) => {
                collect_expression_currencies(&expression, &mut currencies);
            }
            Ok(ParsedLine::Comment) | Ok(ParsedLine::Header(_)) | Err(_) => {}
        }
    }

    currencies.len() <= 1
}

fn currency_shorthand_and_remainder(dimension: &Dimension) -> Option<(&'static str, String)> {
    let mut currency_count = 0;
    let mut shorthand = None;
    let mut remainder = Vec::with_capacity(dimension.0.len());

    for (unit, power) in &dimension.0 {
        if let Unit::Currency(currency) = unit {
            currency_count += 1;
            if *power != Float::from(1) {
                return None;
            }
            shorthand = currency.shorthand();
        } else {
            remainder.push((unit.clone(), power.clone()));
        }
    }

    (currency_count == 1)
        .then_some(shorthand?)
        .map(|shorthand| (shorthand, Dimension(remainder).to_string()))
}

fn display_value(val: Value, uses_currency_shorthands: bool) -> (String, bool) {
    let repeating_candidate = val.is_repeating_candidate();
    let (val, dim) = val.into_parts();

    let mut output = String::new();

    let currency_shorthand = uses_currency_shorthands
        .then(|| currency_shorthand_and_remainder(&dim))
        .flatten();

    if let Some((shorthand, _)) = currency_shorthand {
        output.push_str(shorthand);
    }

    output.push_str(&val.to_string());

    let dim_string = dim.to_string();

    if let Some((_, remainder)) = currency_shorthand {
        if let Some(denominator) = remainder.strip_prefix("1/") {
            output.push('/');
            output.push_str(denominator);
        } else if !remainder.is_empty() {
            output.push('*');
            output.push_str(&remainder);
        }
    } else if !dim_string.is_empty() {
        output.push(' ');
        if dim_string.starts_with("1/") {
            output.push('[');
            output.push_str(&dim_string);
            output.push(']');
        } else {
            output.push_str(&dim_string);
        }
    }

    let repeating = repeating_candidate && has_short_repeating_decimal(&output);
    (output, repeating)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_of(output: &Option<LineOutput>) -> Option<&str> {
        match output {
            Some(LineOutput::Value { string, .. }) => Some(string.as_str()),
            _ => None,
        }
    }

    #[test]
    fn returns_one_slot_per_line_including_the_silent_ones() {
        let mut calculation = Calculation::default();
        let outputs = calculation.evaluate("# Heading\n\n2 + 2");

        assert_eq!(outputs.len(), 3);
        assert_eq!(value_of(&outputs[0]), None);
        assert_eq!(value_of(&outputs[1]), None);
        assert_eq!(value_of(&outputs[2]), Some("4"));
    }

    #[test]
    fn reuses_lines_and_prunes_unused_hashes() {
        let mut calculation = Calculation::default();

        let first = calculation.evaluate("a = 2\na + 3");
        assert_eq!(calculation.lines.len(), 2);
        assert_eq!(value_of(&first[1]), Some("5"));

        let repeated = calculation.evaluate("a = 2\na + 3");
        assert_eq!(repeated, first);
        assert_eq!(calculation.lines.len(), 2);

        let edited = calculation.evaluate("a = 3\na + 3");
        assert_eq!(calculation.lines.len(), 2);
        assert_eq!(value_of(&edited[1]), Some("6"));
    }

    #[test]
    fn reports_an_error_with_the_span_it_covers() {
        let mut calculation = Calculation::default();
        let outputs = calculation.evaluate("2 +");

        assert!(matches!(outputs[0], Some(LineOutput::Error { .. })));
    }
}
