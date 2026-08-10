use super::*;

const MAX_COMPOUNDS_PER_YEAR: i32 = 1_000_000;

pub(super) const DEFINITIONS: &[Function] = siffra_functions![
    {
        name: "apy",
        aliases: ["ear"],
        category: "Finance",
        params: (apr, compounds),
        description: "Annual percentage yield (effective annual rate) from an APR and compounding periods per year. Accepts a percentage or decimal APR.",
        examples: ["apy(5%, 12)" => "5.11619 %"],
        eval: apy,
    },
    {
        name: "apr",
        category: "Finance",
        params: (apy, compounds),
        description: "Nominal annual percentage rate corresponding to an APY and compounding frequency.",
        examples: ["apr(5.11619%, 12)" => "5 %"],
        eval: apr,
    },
    {
        name: "fv",
        aliases: ["compound"],
        category: "Finance",
        params: (principal, rate, periods),
        description: "Future value after compounding a principal at a per-period rate.",
        examples: ["fv(1000 USD, 5%, 10)" => "1,628.894627 USD", "compound($10, 11%, 20)" => "$80.62"],
        eval: future_value,
    },
    {
        name: "pv",
        category: "Finance",
        params: (future, rate, periods),
        description: "Present value of a future amount discounted at a per-period rate.",
        examples: ["pv(1628.894627 USD, 5%, 10)" => "1,000 USD"],
        eval: present_value,
    },
    {
        name: "pmt",
        category: "Finance",
        params: (principal, rate, periods),
        description: "Equal end-of-period payment needed to amortize a principal.",
        examples: ["pmt(1000 USD, 5%, 12)" => "112.82541 USD"],
        eval: payment,
    },
];

fn apy(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (rate, rate_was_percent) = decimal_rate(call.arg(0), "APR", call.span())?;
    let compounds = positive_periods(call.arg(1), "Compounds per year", "APY Error", call.span())?;
    let n = Float::from(compounds);
    let one = Float::from(1);
    let periodic_base = &one + &(&rate / &n);
    if periodic_base <= Float::from(0) {
        return Err(siffra_error!(
            "APY Error",
            "The APR is too negative for this compounding frequency",
            call.span()
        ));
    }
    let growth = periodic_base.pow(&n);
    let annual_yield = (&growth - &one).round_inexact();
    Ok(rate_result(annual_yield, rate_was_percent))
}

fn apr(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (yield_rate, yield_was_percent) = decimal_rate(call.arg(0), "APY", call.span())?;
    let compounds = positive_periods(call.arg(1), "Compounds per year", "APR Error", call.span())?;
    let n = Float::from(compounds);
    let one = Float::from(1);
    let annual_base = &one + &yield_rate;
    if annual_base <= Float::from(0) {
        return Err(siffra_error!(
            "APR Error",
            "APY must be greater than -100%",
            call.span()
        ));
    }
    let root = annual_base.pow(&(&one / &n));
    let nominal_rate = (&n * &(&root - &one)).round_inexact();
    Ok(rate_result(nominal_rate, yield_was_percent))
}

fn future_value(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (base, _, periods) = compounding(call)?;
    finance_result(
        call,
        &call.arg(0).value() * &base.pow(&Float::from(periods)),
    )
}

fn present_value(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (base, _, periods) = compounding(call)?;
    let growth = base.pow(&Float::from(periods));
    if growth == Float::from(0) {
        return Err(siffra_error!(
            "Finance Error",
            "Present value would require division by zero",
            call.span()
        ));
    }
    finance_result(call, &call.arg(0).value() / &growth)
}

fn payment(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (base, rate, periods) = compounding(call)?;
    if rate == Float::from(0) {
        return finance_result(call, &call.arg(0).value() / &Float::from(periods));
    }
    let discount = base.pow(&Float::from(-periods)).round_inexact();
    let denominator = &Float::from(1) - &discount;
    if denominator == Float::from(0) {
        return Err(siffra_error!(
            "Finance Error",
            "Payment cannot be calculated for this rate and period count",
            call.span()
        ));
    }
    finance_result(call, &(&call.arg(0).value() * &rate) / &denominator)
}

/// The shared reading of `(_, rate, periods)`: the per-period growth factor, the
/// rate it came from, and the period count.
fn compounding(call: &Call) -> Result<(Float, Float, i32), SiffraExecutionError> {
    let (rate, _) = decimal_rate(call.arg(1), "rate", call.span())?;
    let periods = positive_periods(call.arg(2), "Periods", "Finance Error", call.span())?;
    let base = &Float::from(1) + &rate;
    if base <= Float::from(0) {
        return Err(siffra_error!(
            "Finance Error",
            "The per-period rate must be greater than -100%",
            call.span()
        ));
    }
    Ok((base, rate, periods))
}

// Money keeps the unit it was given.
fn finance_result(call: &Call, value: Float) -> Result<Value, SiffraExecutionError> {
    Ok(Value::new(
        value.round_inexact(),
        Some(call.arg(0).dimension()),
    ))
}

pub fn positive_periods(
    value: &Value,
    label: &str,
    error_title: &str,
    span: Option<crate::parser::Span>,
) -> Result<i32, SiffraExecutionError> {
    if !value.dimension().is_unitless() {
        return Err(siffra_error!(
            "Unit Error",
            format!("{label} must be unitless; got {}", value_unit(value)),
            span
        ));
    }
    let raw = value.value();
    bounded_integer(&raw, 1, MAX_COMPOUNDS_PER_YEAR).ok_or_else(|| {
        siffra_error!(
            error_title,
            format!("{label} must be an integer from 1 through {MAX_COMPOUNDS_PER_YEAR}"),
            span
        )
    })
}
