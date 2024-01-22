mod state;
pub use state::*;

use pest::Parser;
use rug::{Assign, Float};
use rug::float::Round;
pub use state::SiffraState;

use crate::grammar::representation::ParsedLine;
use crate::grammar::{parse_line, Rule, SiffraParser};
use crate::representations::{Dimension, Expression, Value};

const PI_STRING: &str = "3.14159265358979323846264338327950288419716939937510582097494459230781640628620899862803482534211706798214808651328230";
const E_STRING: &str = "2.718281828459045235360287471352662497757247093699959574966967627724076630353547594571382178525166427427466391932003059";

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
                    "pi" => Ok(Value::new(
                        {
                            let mut num = Float::new(128);
                            num.assign(Float::parse(PI_STRING).unwrap());
                            num
                        },
                        None,
                    )),
                    "e" => Ok(Value::new(
                        {
                            let mut num = Float::new(128);
                            num.assign(Float::parse(E_STRING).unwrap());
                            num
                        },
                        None,
                    )),
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
                    if args.len() == 1 {
                        Ok(Value::new(
                            args[0].value.clone().gamma() * args[0].value.clone(),
                            None,
                        ))
                    } else {
                        Err(())
                    }
                }
                "log" => {
                    if args.len() == 1 {
                        Ok(Value::new(args[0].value.clone().log10(), None))
                    } else if args.len() == 2 {
                        Ok(Value::new(
                            args[1].value.clone().ln() / args[0].value.clone().ln(),
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
                        Ok(args[0].try_pow(&Value::new(Float::with_val(128, 0.5), None)).ok_or(())?)
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
