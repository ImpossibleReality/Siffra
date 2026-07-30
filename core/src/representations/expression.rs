use crate::parser::Span;
use crate::representations::Dimension;
use crate::representations::Value;

#[derive(Debug, Clone)]
pub struct Expression {
    pub inner: InnerExpression,
    pub span: Option<Span>,
    pub function_name_span: Option<Span>,
}

#[derive(Debug, Clone)]
pub enum InnerExpression {
    Constant(Value),
    Negate(Box<Expression>),
    Percent(Box<Expression>),
    Convert(Box<Expression>, Dimension),
    Variable(String),
    FunctionCall { name: String, args: Vec<Expression> },
    Multiply(Box<Expression>, Box<Expression>),
    Divide(Box<Expression>, Box<Expression>),
    Add(Box<Expression>, Box<Expression>),
    Subtract(Box<Expression>, Box<Expression>),
    Exponent(Box<Expression>, Box<Expression>),
    PercentageOperation {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        operator: PercentageOperator,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentageOperator {
    On,
    Off,
    RelativeOf,
    RelativeOn,
    RelativeOff,
    OfWhat,
    OnWhat,
    OffWhat,
}

impl Expression {
    pub fn new(inner: InnerExpression) -> Expression {
        Expression {
            inner,
            span: None,
            function_name_span: None,
        }
    }

    pub fn constant(value: Value) -> Expression {
        Expression::new(InnerExpression::Constant(value))
    }

    pub fn percent(expression: Expression) -> Expression {
        Expression::new(InnerExpression::Percent(Box::new(expression)))
    }

    pub fn negate(expression: Expression) -> Expression {
        Expression::new(InnerExpression::Negate(Box::new(expression)))
    }

    pub fn convert(expression: Expression, dimension: Dimension) -> Expression {
        Expression::new(InnerExpression::Convert(Box::new(expression), dimension))
    }

    pub fn variable(name: String) -> Expression {
        Expression::new(InnerExpression::Variable(name))
    }

    pub fn function_call(name: String, args: Vec<Expression>) -> Expression {
        Expression::new(InnerExpression::FunctionCall { name, args })
    }

    pub fn multiply(lhs: Expression, rhs: Expression) -> Expression {
        Expression::new(InnerExpression::Multiply(Box::new(lhs), Box::new(rhs)))
    }

    pub fn divide(lhs: Expression, rhs: Expression) -> Expression {
        Expression::new(InnerExpression::Divide(Box::new(lhs), Box::new(rhs)))
    }

    pub fn add(lhs: Expression, rhs: Expression) -> Expression {
        Expression::new(InnerExpression::Add(Box::new(lhs), Box::new(rhs)))
    }

    pub fn subtract(lhs: Expression, rhs: Expression) -> Expression {
        Expression::new(InnerExpression::Subtract(Box::new(lhs), Box::new(rhs)))
    }

    pub fn exponent(lhs: Expression, rhs: Expression) -> Expression {
        Expression::new(InnerExpression::Exponent(Box::new(lhs), Box::new(rhs)))
    }

    pub fn percentage_operation(
        lhs: Expression,
        rhs: Expression,
        operator: PercentageOperator,
    ) -> Expression {
        Expression::new(InnerExpression::PercentageOperation {
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            operator,
        })
    }

    pub fn with_span(mut self, span: Span) -> Expression {
        self.span = Some(span);
        self
    }

    pub fn with_function_name_span(mut self, span: Span) -> Expression {
        self.function_name_span = Some(span);
        self
    }

    pub fn span(&self) -> Option<Span> {
        self.span
    }

    pub fn function_name_span(&self) -> Option<Span> {
        self.function_name_span
    }

    pub fn inner(&self) -> &InnerExpression {
        &self.inner
    }
}
