pub mod representation;
mod span;

pub use span::Span;

use lazy_static;
use pest::iterators::{Pair, Pairs};
use representation::*;

use pest::pratt_parser::PrattParser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "src/grammar/siffra.pest"]
pub struct SiffraParser;

lazy_static::lazy_static! {
    static ref PRATT_PARSER: PrattParser<Rule> = {
        use pest::pratt_parser::{Assoc::*, Op};
        use Rule::*;

        // Precedence is defined lowest to highest
        PrattParser::new()
            // Addition and subtract have equal precedence
            .op(Op::infix(add, Left) | Op::infix(subtract, Left))
            .op(Op::infix(multiply, Left)
                | Op::infix(divide, Left)
                | Op::prefix(negative)
                | Op::postfix(convert)
                | Op::postfix(percent))
            .op(Op::infix(exponent, Right))
            .op(Op::postfix(factorial))
    };
}

pub fn parse_unit_expr(pair: Pair<Rule>) -> ParsedDimension {
    let mut pairs = pair.clone().into_inner();
    let numerator = pairs.find_first_tagged("numerator");
    let denominator = pairs.find_first_tagged("denominator");

    let mut units = ParsedDimension {
        numerator: vec![],
        denominator: vec![],
        span: pair.as_span().into(),
    };

    fn parse_mul_group(pair: Pair<Rule>, array: &mut Vec<(ParsedUnit, i32)>) {
        if pair.as_str() != "1" {
            pair.into_inner().for_each(|pair| {
                let unit = pair.clone().into_inner().find_first_tagged("unit").unwrap();
                let power: i32 = pair
                    .clone()
                    .into_inner()
                    .find_first_tagged("power")
                    .map_or(1, |pair| pair.as_str().parse().unwrap());

                if unit
                    .clone()
                    .into_inner()
                    .find_first_tagged("name")
                    .is_some()
                {
                    // Unit has chemical
                    let chemical = unit
                        .clone()
                        .into_inner()
                        .find_first_tagged("chemical")
                        .unwrap()
                        .as_str()
                        .to_string();
                    let name = unit
                        .clone()
                        .into_inner()
                        .find_first_tagged("name")
                        .unwrap()
                        .as_str()
                        .to_string();

                    array.push((
                        ParsedUnit {
                            name,
                            chemical: Some(chemical),
                            span: unit.as_span().into(),
                        },
                        power,
                    ));
                } else {
                    // Unit has no chemical
                    let name = unit.as_str().to_string();

                    array.push((
                        ParsedUnit {
                            name,
                            chemical: None,
                            span: unit.as_span().into(),
                        },
                        power,
                    ));
                }
            });
        }
    }

    if let Some(numerator) = numerator {
        parse_mul_group(numerator, &mut units.numerator);
    }

    if let Some(denominator) = denominator {
        parse_mul_group(denominator, &mut units.denominator);
    }

    if units.numerator.is_empty() && units.denominator.is_empty() {
        if let Some(mul_group) = pairs
            .clone()
            .find(|pair| pair.as_rule() == Rule::unit_mul_group)
        {
            parse_mul_group(mul_group, &mut units.numerator);
        } else if let Some(unit) = pairs.find(|pair| pair.as_rule() == Rule::ungrouped_unit_atom) {
            if unit
                .clone()
                .into_inner()
                .find_first_tagged("name")
                .is_some()
            {
                // Unit has chemical.rs
                let chemical = unit
                    .clone()
                    .into_inner()
                    .find_first_tagged("chemical")
                    .unwrap()
                    .as_str()
                    .to_string();
                let name = unit
                    .clone()
                    .into_inner()
                    .find_first_tagged("name")
                    .unwrap()
                    .as_str()
                    .to_string();

                units.numerator.push((
                    ParsedUnit {
                        name,
                        chemical: Some(chemical),
                        span: unit.as_span().into(),
                    },
                    1,
                ));
            } else {
                // Unit has no chemical.rs
                let name = unit.as_str().to_string();

                units.numerator.push((
                    ParsedUnit {
                        name,
                        chemical: None,
                        span: unit.as_span().into(),
                    },
                    1,
                ));
            }
        }
    }

    units
}

pub fn parse_expr(pairs: Pairs<Rule>) -> ParsedExpr {
    PRATT_PARSER
        .map_primary(|primary| match primary.as_rule() {
            Rule::expr => parse_expr(primary.into_inner()),
            Rule::number => ParsedExpr::Number {
                value: primary.as_str().parse().unwrap(),
                units: None,
                span: primary.as_span().into(),
            },
            Rule::dimensional_number => ParsedExpr::Number {
                value: primary
                    .clone()
                    .into_inner()
                    .find(|pair| pair.as_rule() == Rule::number)
                    .unwrap()
                    .as_str()
                    .parse()
                    .unwrap(),
                units: primary
                    .clone()
                    .into_inner()
                    .find(|pair| pair.as_rule() == Rule::units_expr)
                    .map(|pair| parse_unit_expr(pair)),
                span: primary.as_span().into(),
            },
            Rule::variable => ParsedExpr::Variable {
                name: primary.as_str().to_string(),
                span: primary.as_span().into(),
            },
            Rule::ungrouped_function => {
                let primary_span = primary.as_span();
                let inner = primary.into_inner();
                let name_pair = inner.find_first_tagged("name").unwrap();

                let name = name_pair.as_str().to_string();
                let name_span = name_pair.as_span();

                let arg = inner.find_first_tagged("input").unwrap();
                ParsedExpr::FunctionCall {
                    name,
                    args: vec![parse_expr(Pairs::single(arg))],
                    base: None,
                    span: primary_span.into(),
                    function_span: name_span.into(),
                }
            }
            Rule::grouped_function | Rule::base_function => {
                let primary_span = primary.as_span();
                let inner = primary.into_inner();
                let name_pair = inner.find_first_tagged("name").unwrap();
                let name = name_pair.as_str().to_string();
                let name_span = name_pair.as_span();

                let args = inner
                    .clone()
                    .find(|pair| pair.as_rule() == Rule::function_input)
                    .unwrap()
                    .into_inner()
                    .map(|p| parse_expr(Pairs::single(p)))
                    .collect();
                let base = inner
                    .find_first_tagged("base")
                    .map(|p| Box::new(parse_expr(Pairs::single(p))));
                ParsedExpr::FunctionCall {
                    name,
                    args,
                    base,
                    span: primary_span.into(),
                    function_span: name_span.into(),
                }
            }
            Rule::grouped_mul_atom => {
                let primary_span = primary.as_span();
                let inner = primary.into_inner();

                let mut pairs = inner.peekable();
                let mut expr = parse_expr(Pairs::single(pairs.next().unwrap()));
                while pairs.peek().is_some() {
                    expr = ParsedExpr::BinOp {
                        lhs: Box::new(expr),
                        op: Op::Multiply,
                        rhs: Box::new(parse_expr(Pairs::single(pairs.next().unwrap()))),
                        span: primary_span.into(),
                    };
                }

                expr
            }
            rule => unreachable!("Expr::parse expected atom, found {:?}", rule),
        })
        .map_infix(|lhs, op_pairs, rhs| {
            let op = match op_pairs.as_rule() {
                Rule::add => Op::Add,
                Rule::subtract => Op::Subtract,
                Rule::multiply => Op::Multiply,
                Rule::divide => Op::Divide,
                Rule::exponent => Op::Exponent,
                rule => unreachable!("Expr::parse expected infix operation, found {:?}", rule),
            };
            ParsedExpr::BinOp {
                lhs: Box::new(lhs),
                op,
                rhs: Box::new(rhs),
                span: op_pairs.as_span().into(),
            }
        })
        .map_postfix(|lhs, op_pairs| {
            let op_span = op_pairs.as_span();
            let op = match op_pairs.as_rule() {
                Rule::factorial => OpPost::Factorial,
                Rule::percent => OpPost::Percent,
                Rule::convert => OpPost::Convert(parse_unit_expr(
                    op_pairs
                        .into_inner()
                        .find(|pair| pair.as_rule() == Rule::units_expr)
                        .unwrap(),
                )),
                rule => unreachable!("Expr::parse expected postfix operation, found {:?}", rule),
            };
            ParsedExpr::UnOpPost {
                lhs: Box::new(lhs),
                op,
                span: op_span.into(),
            }
        })
        .map_prefix(|op_pairs, rhs| {
            let op = match op_pairs.as_rule() {
                Rule::negative => OpPre::Negate,
                rule => unreachable!("Expr::parse expected prefix operation, found {:?}", rule),
            };
            ParsedExpr::UnOpPre {
                op,
                rhs: Box::new(rhs),
                span: op_pairs.as_span().into(),
            }
        })
        .parse(pairs)
}

pub fn parse_line(mut pairs: Pairs<Rule>) -> ParsedLine {
    if let Some(pair) = pairs
        .clone()
        .find(|pair| pair.as_rule() == Rule::variable_constraint)
    {
        let mut inner = pair.into_inner();
        let name = inner
            .find(|pair| pair.as_rule() == Rule::variable)
            .unwrap()
            .as_str();
        let expr = parse_expr(
            inner
                .find(|pair| pair.as_rule() == Rule::expr)
                .unwrap()
                .into_inner(),
        );
        return ParsedLine::Variable(name.to_string(), expr);
    }

    if let Some(pair) = pairs.find(|pair| pair.as_rule() == Rule::expr) {
        let expr = parse_expr(pair.into_inner());
        return ParsedLine::Expression(expr);
    }

    ParsedLine::Comment
}

#[cfg(test)]
mod tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn test_parse_expr() {
        let _expr = parse_expr(SiffraParser::parse(Rule::expr, "log2(5(x)(y)) as mol %").unwrap());
        let _expr = parse_expr(SiffraParser::parse(Rule::expr, "a times 2% of 3").unwrap());
    }

    #[test]
    fn test_parse_line() {
        let line = parse_line(SiffraParser::parse(Rule::line, "x = 5").unwrap());
        assert!(matches!(line, ParsedLine::Variable(_, _)));
        let line = parse_line(SiffraParser::parse(Rule::line, "log2(5(x)(y)) as mol %").unwrap());
        assert!(matches!(line, ParsedLine::Expression(_)));
        let line = parse_line(SiffraParser::parse(Rule::line, "// This is a comment").unwrap());
        assert!(matches!(line, ParsedLine::Comment));
        let line = parse_line(SiffraParser::parse(Rule::line, "/* This is a comment */").unwrap());
        assert!(matches!(line, ParsedLine::Comment));
    }

    #[test]
    fn test_ungrouped_functions() {
        let _expr = parse_expr(SiffraParser::parse(Rule::expr, "log 5a").unwrap());
    }

    #[test]
    fn test_nested_functions() {
        let expr = parse_expr(SiffraParser::parse(Rule::expr, "sin(cos(5))").unwrap());
        assert_eq!(
            ParsedExpr::FunctionCall {
                name: "sin".to_string(),
                args: vec![ParsedExpr::FunctionCall {
                    name: "cos".to_string(),
                    args: vec![ParsedExpr::Number {
                        value: "5".to_string(),
                        units: None,
                        span: Span::new(8, 9),
                    }],
                    base: None,
                    span: Span::new(4, 10),
                    function_span: Span::new(4, 7),
                }],
                base: None,
                span: Span::new(0, 11),
                function_span: Span::new(0, 3),
            },
            expr
        );
    }
}
