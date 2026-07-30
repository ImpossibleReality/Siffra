use crate::{quantity, ratio, si_prefixable};

// Prefixes such as `mmol`, `kmol`, and `umol` come from the SI prefix table.
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

// The "gram mole" spellings take a symbol prefix directly (`kgmol`, `mgmol`),
// the way chemical engineering writes them.
si_prefixable!(Amount, [Mole ["gmol", "gmole", "gmoles"]]);
