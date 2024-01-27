mod state;

use pest::Parser;
pub use state::SiffraState;

use crate::grammar::representation::ParsedLine;
use crate::grammar::{parse_line, Rule, SiffraParser};
use crate::representations::{Dimension, Expression, Float, Value};
pub type EvaluationResult = Result<Option<Value>, ()>;

pub fn evaluate_line(line: &str, state: &mut SiffraState) -> EvaluationResult {
    let pairs = SiffraParser::parse(Rule::line, line).map_err(|_| ())?;
    let parsed_line = parse_line(pairs);
    match parsed_line {
        ParsedLine::Comment => Ok(None),
        ParsedLine::Expression(expr) => {
            let value = evaluate_expr(&expr.try_into()?, state)?;
            Ok(Some(value))
        }
        ParsedLine::Variable(name, expr) => {
            let value = evaluate_expr(&expr.try_into()?, state)?;
            state.set_variable(&name, value.clone());
            Ok(Some(value))
        }
    }
}

pub fn evaluate_expr(expr: &Expression, state: &SiffraState) -> Result<Value, ()> {
    match expr {
        Expression::Constant(name) => Ok(name.clone()),
        Expression::Convert(val, dim) => {
            let val = evaluate_expr(val, state)?;
            if dim.is_unitless() {
                return Ok(val.with_units(Dimension::default()));
            } else if val.dimension().is_unitless() {
                return Ok(val.with_units(dim.clone()));
            } else {
                Ok(val.convert(dim).ok_or(())?)
            }
        }
        Expression::Variable(name) => {
            if let Some(v) = state.get_variable(name) {
                Ok(v.clone())
            } else {
                match name.as_str() {
                    "pi" => Ok(Value::new(Float::pi(), None)),
                    "e" => Ok(Value::new(Float::e(), None)),
                    _ => Err(()),
                }
            }
        }
        Expression::FunctionCall { name, args } => {
            let args = args
                .iter()
                .map(|arg| evaluate_expr(arg, state))
                .collect::<Result<Vec<_>, _>>()?;
            match &**name {
                "factorial" => {
                    // TODO: Implement gamma function
                    Err(())
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
                        Err(())
                    }
                }
                "ln" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().ln(), None))
                    } else {
                        Err(())
                    }
                }
                "sqrt" => {
                    if args.len() == 1 {
                        Ok(args[0]
                            .try_pow(&Value::new(Float::from(0.5), None))
                            .ok_or(())?)
                    } else {
                        Err(())
                    }
                }
                "sin" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().sin(), None))
                    } else {
                        Err(())
                    }
                }
                "cos" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().cos(), None))
                    } else {
                        Err(())
                    }
                }
                "tan" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().tan(), None))
                    } else {
                        Err(())
                    }
                }
                "asin" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().asin(), None))
                    } else {
                        Err(())
                    }
                }
                "acos" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().acos(), None))
                    } else {
                        Err(())
                    }
                }
                "atan" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().atan(), None))
                    } else {
                        Err(())
                    }
                }

                _ => Err(()),
            }
        }
        Expression::Multiply(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(a.try_mul(&b).ok_or(())?)
        }
        Expression::Divide(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(a.try_div(&b).ok_or(())?)
        }
        Expression::Add(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(a.try_add(&b).ok_or(())?)
        }
        Expression::Subtract(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(a.try_sub(&b).ok_or(())?)
        }
        Expression::Exponent(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(a.try_pow(&b).ok_or(())?)
        }
    }
}
