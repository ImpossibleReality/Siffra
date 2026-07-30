use crate::{quantity, ratio, si_prefixable};

// The ampere is the SI base electrical quantity; derived electrical units use
// it, and scaled amperes come from the SI prefix table.
quantity!(
    ElectricCurrent,
    [(
        Ampere,
        ratio!(1 / 1),
        "A",
        "ampere",
        "amperes",
        "amp",
        "amps"
    )]
);

si_prefixable!(ElectricCurrent, [Ampere]);
