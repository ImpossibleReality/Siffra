use crate::{quantity, ratio, si_prefixable};

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
