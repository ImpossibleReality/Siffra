use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::parser::{ParsedDimension, ParsedExpr, ParsedLine, parse_line};
use siffra::representations::{
    Currency, Dimension, Float, FormattedValue, Unit, Value, format_value,
};
use std::collections::{BTreeSet, HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq)]
pub enum LineOutput {
    Value {
        string: String,
        repeating: bool,
        /// Only decimal numbers may be rounded to fit the result column;
        /// instants and reading lists have shapes of their own.
        numeric: bool,
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
        let timezone = state.timezone().clone();
        let mut outputs = Vec::new();
        let mut previous_hash = 0;
        let mut used_hashes = HashSet::new();
        let (uses_currency_shorthands, first_volatile) = document_flags(input);

        for (index, line) in input.split('\n').enumerate() {
            let hash = line_hash(line, previous_hash);
            used_hashes.insert(hash);

            // A line reading the clock, and every line whose state depends on
            // it, has to be recomputed rather than answered from the cache.
            let cacheable = first_volatile.is_none_or(|first| index < first);
            let cached = cacheable
                .then(|| {
                    self.lines.get(&hash).filter(|entry| {
                        entry.line == line
                            && entry.previous_hash == previous_hash
                            && entry.uses_currency_shorthands == uses_currency_shorthands
                    })
                })
                .flatten()
                .cloned();

            let output = if let Some(cached) = cached {
                state = cached.state_after;
                cached.output
            } else {
                let output = match evaluate_line(line, &mut state) {
                    Ok(Some(value)) => {
                        let formatted = format_value(&value, Some(&timezone));
                        let numeric = matches!(formatted, FormattedValue::Number { .. });
                        let (string, repeating) =
                            display_value(&value, formatted, uses_currency_shorthands);
                        Some(LineOutput::Value {
                            string,
                            repeating,
                            numeric,
                        })
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

                if cacheable {
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
                }
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
        ParsedExpr::Instant { .. } | ParsedExpr::ClockTime { .. } => {}
        // TODO: Dollars + cents?
        ParsedExpr::Mixed { .. } => {}
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

/// A line that reads the clock cannot be cached, and neither can the lines
/// after it, whose state it feeds.
fn expression_reads_the_clock(expression: &ParsedExpr) -> bool {
    match expression {
        ParsedExpr::Instant { instant, .. } => instant.keyword.is_some(),
        // A wall clock is anchored to today's date
        ParsedExpr::ClockTime { .. } => true,
        ParsedExpr::Number { .. } | ParsedExpr::Mixed { .. } | ParsedExpr::Variable { .. } => false,
        ParsedExpr::FunctionCall { args, base, .. } => {
            base.as_deref().is_some_and(expression_reads_the_clock)
                || args.iter().any(expression_reads_the_clock)
        }
        ParsedExpr::UnOpPre { rhs, .. } => expression_reads_the_clock(rhs),
        ParsedExpr::UnOpPost { lhs, .. } => expression_reads_the_clock(lhs),
        ParsedExpr::BinOp { lhs, rhs, .. } => {
            expression_reads_the_clock(lhs) || expression_reads_the_clock(rhs)
        }
    }
}

/// Whether currency shorthand applies, and the first line (if any) from which
/// results stop being cacheable.
fn document_flags(input: &str) -> (bool, Option<usize>) {
    let mut currencies = BTreeSet::new();
    let mut first_volatile = None;

    for (index, line) in input.split('\n').enumerate() {
        match parse_line(line) {
            Ok(ParsedLine::Expression(expression) | ParsedLine::Variable(_, expression)) => {
                collect_expression_currencies(&expression, &mut currencies);
                if first_volatile.is_none() && expression_reads_the_clock(&expression) {
                    first_volatile = Some(index);
                }
            }
            Ok(ParsedLine::Comment) | Ok(ParsedLine::Header(_)) | Err(_) => {}
        }
    }

    (currencies.len() <= 1, first_volatile)
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

fn display_value(
    value: &Value,
    formatted: FormattedValue,
    uses_currency_shorthands: bool,
) -> (String, bool) {
    // Instants and reading lists are already rendered; only plain numbers get
    // the contextual currency shorthand.
    let FormattedValue::Number { text, unit } = &formatted else {
        return (formatted.to_plain_string(), false);
    };

    let repeating_candidate = value.is_repeating_candidate();
    let dim = value.dimension();

    let mut output = String::new();

    let currency_shorthand = uses_currency_shorthands
        .then(|| currency_shorthand_and_remainder(&dim))
        .flatten();

    if let Some((shorthand, _)) = currency_shorthand {
        output.push_str(shorthand);
    }

    output.push_str(text);

    let dim_string = unit.clone().unwrap_or_default();

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
    fn a_line_that_reads_the_clock_is_never_answered_from_the_cache() {
        let mut calculation = Calculation::default();

        // The clock line and everything downstream of its state are recomputed;
        // the lines above it still cache normally.
        calculation.evaluate("a = 2\nnow\na + 1");
        assert_eq!(calculation.lines.len(), 1);
        assert!(
            calculation
                .lines
                .values()
                .all(|entry| entry.line == "a = 2")
        );

        calculation.evaluate("a = 2\n2 + 2\na + 1");
        assert_eq!(calculation.lines.len(), 3);
    }

    #[test]
    fn dates_and_spans_are_marked_as_non_numeric_output() {
        let mut calculation = Calculation::default();
        let outputs = calculation.evaluate("2024-02-10\n2024-02-10 to 2024-03-10\n2 + 2");

        assert!(matches!(
            outputs[0],
            Some(LineOutput::Value { numeric: false, .. })
        ));
        assert!(matches!(
            outputs[1],
            Some(LineOutput::Value { numeric: false, .. })
        ));
        assert!(matches!(
            outputs[2],
            Some(LineOutput::Value { numeric: true, .. })
        ));
    }

    #[test]
    fn a_ladder_is_rendered_whole_rather_than_rounded_as_a_decimal() {
        let mut calculation = Calculation::default();
        let outputs = calculation.evaluate("5 ft 2 in\n1.87 m to ft,in");

        assert_eq!(value_of(&outputs[0]), Some("5 ft 2 in"));
        assert_eq!(value_of(&outputs[1]), Some("6 ft 1.622 in"));
        assert!(
            outputs
                .iter()
                .all(|output| matches!(output, Some(LineOutput::Value { numeric: false, .. })))
        );
    }

    #[test]
    fn reports_an_error_with_the_span_it_covers() {
        let mut calculation = Calculation::default();
        let outputs = calculation.evaluate("2 +");

        assert!(matches!(outputs[0], Some(LineOutput::Error { .. })));
    }
}
