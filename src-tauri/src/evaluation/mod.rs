pub struct EvaluationResult(Value);

pub fn evaluate_line(line: &str, state: &mut TaleState) -> EvaluationResult {
    let mut parser = Parser::new(line);
    let expression = parser.parse_expression();
    let value = expression.evaluate(state);
    EvaluationResult(value)
}