use crate::{quantity, ratio, si_prefixable};

quantity!(
    Amount,
    [
        (
            Mole,
            ratio!(1 / 1),
            "mol",
            "mole",
            "moles",
            "gmol",
            "gmole",
            "gmoles"
        ),
        (
            PoundMole,
            ratio!(45_359_237 / 100_000),
            "lbmol",
            "pound-mole",
            "pound-moles"
        )
    ]
);

si_prefixable!(Amount, [Mole ["gmol", "gmole", "gmoles"]]);
