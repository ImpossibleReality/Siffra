use crate::representations::Unit;

/// How much of an instant is meaningful to show. Ordered coarse to fine so
/// combining two hints is a maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Granularity {
    Date,
    Minute,
    Second,
}

impl Granularity {
    pub fn finer(self, other: Self) -> Self {
        self.max(other)
    }
}

/// Rendering metadata retained with a value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayHints {
    /// Temporal points only: how much of the reading to show.
    pub granularity: Option<Granularity>,
    /// Show a calendar basis as a list of readings. Cleared by conversion,
    /// which is a request for one particular reading.
    pub span_readings: bool,
    /// Largest-to-smallest units of one dimensionality to split the value
    /// across, as `5 ft 2 in` splits one length. Unlike `span_readings`, whose
    /// entries are alternative totals, these rungs sum to the value. Calendar
    /// shifts also apply these rungs in order.
    pub ladder: Option<Vec<Unit>>,
    /// The value came from a division and may be a recurring decimal.
    pub repeating_candidate: bool,
}

impl DisplayHints {
    pub fn instant(granularity: Granularity) -> Self {
        Self {
            granularity: Some(granularity),
            ..Self::default()
        }
    }

    pub fn readings() -> Self {
        Self {
            span_readings: true,
            ..Self::default()
        }
    }

    pub fn ladder(units: Vec<Unit>) -> Self {
        Self {
            ladder: Some(units),
            ..Self::default()
        }
    }

    /// Keeps only what survives arithmetic between two operands.
    ///
    /// A ladder on either side is adopted, the left one winning when both have
    /// one; whether it still fits the result is not something two hint sets can
    /// answer, so the operand keeps it only while `Value::checked_ladder` says
    /// the result still measures what the rungs measure.
    pub fn merged(&self, other: &Self) -> Self {
        Self {
            granularity: None,
            span_readings: false,
            ladder: self.ladder.clone().or_else(|| other.ladder.clone()),
            repeating_candidate: self.repeating_candidate || other.repeating_candidate,
        }
    }

    /// Raises the granularity to at least `granularity`.
    pub fn at_least(mut self, granularity: Granularity) -> Self {
        self.granularity = Some(match self.granularity {
            Some(current) => current.finer(granularity),
            None => granularity,
        });
        self
    }
}
