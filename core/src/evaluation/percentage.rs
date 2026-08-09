use super::helpers::value_unit;
use crate::error::SiffraExecutionError;
use crate::parser::Span;
use crate::representations::{Dimension, Float, PercentageOperator, Value};
use crate::siffra_error;

fn percentage_as_scalar(
    value: &Value,
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    if !value.dimension().is_percent() {
        return Err(siffra_error!(
            "Percentage Error",
            format!("Expected a percentage; got {}", value_unit(value)),
            span
        ));
    }
    value.convert(&Dimension::default()).ok_or_else(|| {
        siffra_error!(
            "Percentage Error",
            "Unable to convert the percentage to a scalar",
            span
        )
    })
}

fn relative_percentage(
    numerator: Value,
    denominator: Value,
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    if denominator.value() == Float::from(0) {
        return Err(siffra_error!(
            "Percentage Error",
            "A percentage relative to zero is undefined",
            span
        ));
    }
    let ratio = numerator.try_div(&denominator).ok_or_else(|| {
        siffra_error!(
            "Unit Error",
            format!(
                "Cannot compare {} and {}",
                value_unit(&numerator),
                value_unit(&denominator)
            ),
            span
        )
    })?;
    ratio.convert(&Dimension::percent()).ok_or_else(|| {
        siffra_error!(
            "Unit Error",
            format!(
                "Cannot compare {} and {}",
                value_unit(&numerator),
                value_unit(&denominator)
            ),
            span
        )
    })
}

pub(super) fn evaluate_percentage(
    operator: &PercentageOperator,
    lhs: Value,
    rhs: Value,
    span: Option<Span>,
) -> Result<Value, SiffraExecutionError> {
    match operator {
        PercentageOperator::On => {
            percentage_as_scalar(&lhs, span)?;
            rhs.try_add(&lhs).ok_or_else(|| {
                siffra_error!(
                    "Percentage Error",
                    "Unable to add this percentage to the value",
                    span
                )
            })
        }
        PercentageOperator::Off => {
            percentage_as_scalar(&lhs, span)?;
            rhs.try_sub(&lhs).ok_or_else(|| {
                siffra_error!(
                    "Percentage Error",
                    "Unable to subtract this percentage from the value",
                    span
                )
            })
        }
        PercentageOperator::RelativeOf => relative_percentage(lhs, rhs, span),
        PercentageOperator::RelativeOn => {
            let increase = lhs.try_sub(&rhs).ok_or_else(|| {
                siffra_error!(
                    "Unit Error",
                    format!(
                        "Cannot compare {} and {}",
                        value_unit(&lhs),
                        value_unit(&rhs)
                    ),
                    span
                )
            })?;
            relative_percentage(increase, rhs, span)
        }
        PercentageOperator::RelativeOff => {
            let decrease = rhs.try_sub(&lhs).ok_or_else(|| {
                siffra_error!(
                    "Unit Error",
                    format!(
                        "Cannot compare {} and {}",
                        value_unit(&lhs),
                        value_unit(&rhs)
                    ),
                    span
                )
            })?;
            relative_percentage(decrease, rhs, span)
        }
        PercentageOperator::OfWhat | PercentageOperator::OnWhat | PercentageOperator::OffWhat => {
            let percentage = percentage_as_scalar(&lhs, span)?;
            let one = Float::from(1);
            let divisor = match operator {
                PercentageOperator::OfWhat => percentage.value(),
                PercentageOperator::OnWhat => &one + &percentage.value(),
                PercentageOperator::OffWhat => &one - &percentage.value(),
                _ => unreachable!(),
            };
            if divisor == Float::from(0) {
                return Err(siffra_error!(
                    "Percentage Error",
                    "This inverse percentage calculation would divide by zero",
                    span
                ));
            }
            rhs.try_div(&Value::new(divisor, None)).ok_or_else(|| {
                siffra_error!(
                    "Percentage Error",
                    "Unable to solve this inverse percentage calculation",
                    span
                )
            })
        }
    }
}
