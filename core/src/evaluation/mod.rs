mod state;

use crate::error::SiffraExecutionError;
use pest::error::InputLocation;
use pest::Parser;
pub use state::SiffraState;

use crate::grammar::representation::ParsedLine;
use crate::grammar::{parse_line, Rule, SiffraParser, Span};
use crate::representations::{Dimension, Expression, Float, InnerExpression, Value};
use crate::{siffra_error, siffra_try};
use crate::evaluation::state::VariableAccessError;

pub type EvaluationResult = Result<Option<Value>, SiffraExecutionError>;

pub fn evaluate_line(line: &str, state: &mut SiffraState) -> EvaluationResult {
    let pairs = SiffraParser::parse(Rule::line, line).map_err(|e| match e.location {
        InputLocation::Pos(pos) => siffra_error!(
            "Syntax Error",
            "Could not parse the provided line",
            Some(Span::new(pos, pos + 1))
        ),
        InputLocation::Span((start, end)) => siffra_error!(
            "Syntax Error",
            "Could not parse the provided line",
            Some(Span::new(start, end))
        ),
    })?;
    let parsed_line = parse_line(pairs);
    match parsed_line {
        ParsedLine::Comment => Ok(None),
        ParsedLine::Expression(expr) => {
            let value = evaluate_expr(&expr.try_into()?, state)?;
            Ok(Some(value))
        }
        ParsedLine::Variable(name, expr) => {
            let value = evaluate_expr(&expr.try_into()?, state);
            match value {
                Ok(value) => {
                    state.set_variable(&name, value.clone());
                    Ok(Some(value))
                }
                Err(err) => {
                    state.error_variable(&name, err.clone());
                    Err(err)
                }
            }
        }
    }
}

pub fn evaluate_expr(
    expr: &Expression,
    state: &SiffraState,
) -> Result<Value, SiffraExecutionError> {
    match expr.inner() {
        InnerExpression::Constant(name) => Ok(name.clone()),
        InnerExpression::Convert(val, dim) => {
            let val = evaluate_expr(val, state)?;
            if dim.is_unitless() {
                return Ok(val.with_units(Dimension::default()));
            } else if val.dimension().is_unitless() {
                return Ok(val.with_units(dim.clone()));
            } else {
                Ok(siffra_try!(
                    val.convert(dim).ok_or(()),
                    "Unit Error",
                    "Error converting units",
                    expr.span()
                ))
            }
        }
        InnerExpression::Variable(name) => {
            if let Ok(v) = state.get_variable(name) {
                Ok(v.clone())
            } else if let Err(VariableAccessError::Error(_)) = state.get_variable(name) {
                Err(siffra_error!(
                    "Variable Error",
                    "Definition of variable had an error",
                    expr.span()
                ))
            } else {
                match name.as_str() {
                    "pi" => Ok(Value::new(Float::pi(), None)),
                    "e" => Ok(Value::new(Float::e(), None)),
                    _ => Err(siffra_error!(
                        "Name Error",
                        format!("Variable '{}' not found", name),
                        expr.span()
                    )),
                }
            }
        }
        InnerExpression::FunctionCall { name, args } => {
            let args = args
                .iter()
                .map(|arg| evaluate_expr(arg, state))
                .collect::<Result<Vec<_>, _>>()?;
            match &**name {
                "factorial" => {
                    // TODO: Implement gamma function
                    Err(siffra_error!(
                        "Not Yet Implemented Error",
                        "Factorial function not yet implemented",
                        expr.span()
                    ))
                }
                "log" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().log10(), None))
                    } else if args.len() == 2 {
                        Ok(Value::new(
                            &args[1].value.clone().ln() / &args[0].value.clone().ln(),
                            None,
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "log function takes either 1 or 2 arguments",
                            expr.span()
                        ))
                    }
                }
                "ln" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().ln(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "ln function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "sqrt" => {
                    if args.len() == 1 {
                        Ok(siffra_try!(
                            args[0]
                                .try_pow(&Value::new(Float::from(0.5), None))
                                .ok_or(()),
                            "Root Error",
                            "Error taking square root",
                            expr.span()
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "sqrt function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "sin" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().sin(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "sin function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "cos" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().cos(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "cos function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "tan" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().tan(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "tan function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "asin" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().asin(), None))
                    } else {
                        Err(siffra_error!(
                            "asin function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "acos" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().acos(), None))
                    } else {
                        Err(siffra_error!(
                            "acos function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "atan" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().atan(), None))
                    } else {
                        Err(siffra_error!(
                            "atan function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }

                _ => Err(siffra_error!("Function not defined", expr.span())),
            }
        }
        InnerExpression::Multiply(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_mul(&b).ok_or(()),
                "Unit Error",
                "Units do not match in multiplication",
                expr.span()
            ))
        }
        InnerExpression::Divide(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_div(&b).ok_or(()),
                "Division Error",
                "Division by zero",
                expr.span()
            ))
        }
        InnerExpression::Add(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_add(&b).ok_or(()),
                "Unit Error",
                "Units do not match in addition",
                expr.span()
            ))
        }
        InnerExpression::Subtract(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_sub(&b).ok_or(()),
                "Unit Error",
                "Units do not match in subtraction",
                expr.span()
            ))
        }
        InnerExpression::Exponent(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_pow(&b).ok_or(()),
                "Unit Error",
                "Exponent must be unitless",
                expr.span()
            ))
        }
    }
}
