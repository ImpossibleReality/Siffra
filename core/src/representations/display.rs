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

/// Advisory rendering hints. Dropping them never changes what a value means,
/// so any operation that cannot propagate a hint may simply omit it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayHints {
    /// Temporal points only: how much of the reading to show.
    pub granularity: Option<Granularity>,
    /// Show a calendar basis as a list of readings. Cleared by conversion,
    /// which is a request for one particular reading.
    pub span_readings: bool,
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

    /// Keeps only what survives arithmetic between two operands.
    pub fn merged(&self, other: &Self) -> Self {
        Self {
            granularity: None,
            span_readings: false,
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
