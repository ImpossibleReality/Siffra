//! Browser bindings for the Siffra engine, backing the demo on the website.

use serde::Serialize;
use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::parser::{SyntaxKind, syntax_spans};
use siffra::representations::{
    Currency, Dimension, Float, FormattedValue, NumberDisplay, Unit, Value,
    format_number_for_characters, format_value,
};
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Segment {
    token: Option<&'static str>,
    text: String,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Output {
    Value {
        text: String,
        /// The stretch of `text` the app draws a vinculum over, if it recurs.
        repetend: Option<[usize; 2]>,
    },
    Error {
        message: String,
        description: Option<String>,
    },
}

#[derive(Serialize)]
struct Line {
    segments: Vec<Segment>,
    result: Option<Output>,
}

/// Evaluates a whole document, returning one entry per line. `budget` is the width of the result
/// column in characters, which decides how far a long decimal is rounded.
#[wasm_bindgen]
pub fn evaluate(document: &str, budget: usize) -> Result<JsValue, JsValue> {
    let mut state = SiffraState::new();
    let timezone = state.timezone().clone();
    let uses_currency_shorthands = counts_at_most_one_currency(document);

    let lines: Vec<Line> = document
        .split('\n')
        .map(|line| {
            let result = match evaluate_line(line, &mut state) {
                Ok(Some(value)) => {
                    let formatted = format_value(&value, Some(&timezone));
                    // Only plain decimals are rounded; instants and unit ladders have shapes of
                    // their own that a character budget would break.
                    let numeric = matches!(formatted, FormattedValue::Number { .. });
                    let repeating = value.is_repeating_candidate();
                    let text = display_value(&value, formatted, uses_currency_shorthands);
                    Some(fit(text, numeric, repeating, budget))
                }
                Ok(None) => None,
                Err(err) => Some(Output::Error {
                    message: err.message(),
                    description: err.description(),
                }),
            };

            Line {
                segments: segments(line),
                result,
            }
        })
        .collect();

    serde_wasm_bindgen::to_value(&lines).map_err(Into::into)
}

/// Installs exchange rates relative to the euro. The engine cannot reach the network from wasm, so
/// the page fetches them and hands them over here.
#[wasm_bindgen]
pub fn install_rates(codes: Vec<String>, rates: Vec<f64>) {
    let pairs: Vec<(&str, f64)> = codes
        .iter()
        .map(String::as_str)
        .zip(rates.iter().copied())
        .collect();
    siffra::representations::install_static_rates(&pairs);
}

fn fit(text: String, numeric: bool, repeating: bool, budget: usize) -> Output {
    if !numeric {
        return Output::Value {
            text,
            repetend: None,
        };
    }

    let NumberDisplay {
        before_repeat,
        repeating: repetend,
        after_repeat,
    } = format_number_for_characters(&text, budget, repeating);

    let start = before_repeat.chars().count();
    let end = start + repetend.chars().count();
    Output::Value {
        text: format!("{before_repeat}{repetend}{after_repeat}"),
        repetend: (!repetend.is_empty()).then_some([start, end]),
    }
}

fn token_name(kind: SyntaxKind) -> &'static str {
    match kind {
        SyntaxKind::Header => "header",
        SyntaxKind::Comment => "comment",
        SyntaxKind::Chemical => "chemical",
        SyntaxKind::Number => "number",
        SyntaxKind::Unit => "unit",
        // The app paints function names with its operation colour.
        SyntaxKind::Function | SyntaxKind::Operation => "operation",
        SyntaxKind::Variable => "variable",
    }
}

fn segments(line: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut cursor = 0;

    for span in syntax_spans(line) {
        let (start, end) = (span.span.start(), span.span.end());
        if start > cursor {
            out.push(Segment {
                token: None,
                text: line[cursor..start].to_string(),
            });
        }
        out.push(Segment {
            token: Some(token_name(span.kind)),
            text: line[start..end].to_string(),
        });
        cursor = end;
    }

    if cursor < line.len() {
        out.push(Segment {
            token: None,
            text: line[cursor..].to_string(),
        });
    }

    out
}

/// Currency shorthands (`$`) only read unambiguously while a document sticks to a single currency.
fn counts_at_most_one_currency(document: &str) -> bool {
    let mut seen: Vec<&'static str> = Vec::new();

    for line in document.split('\n') {
        for span in syntax_spans(line) {
            if span.kind != SyntaxKind::Unit {
                continue;
            }
            if let Ok(currency) = Currency::from_str(&line[span.span.start()..span.span.end()])
                && !seen.contains(&currency.code())
            {
                seen.push(currency.code());
            }
        }
    }

    seen.len() <= 1
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
) -> String {
    // Instants and reading lists are already rendered; only plain numbers get the contextual
    // currency shorthand.
    let FormattedValue::Number { text, unit } = &formatted else {
        return formatted.to_plain_string();
    };

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

    output
}
