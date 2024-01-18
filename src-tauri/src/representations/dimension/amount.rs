use crate::{quantity, ratio};

quantity!(
    Amount,
    [
        // SI units - moles are sometimes also referred to as "gram moles"
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
            Kilomole,
            ratio!(1_000 / 1),
            "kmol",
            "kilomole",
            "kilomoles",
            "kgmol",
            "kgmole",
            "kgmoles"
        ),
        (
            Megamole,
            ratio!(1_000_000 / 1),
            "Mmol",
            "megamole",
            "megamoles",
            "Mgmol",
            "Mgmole",
            "Mgmoles"
        ),
        (
            Gigamole,
            ratio!(1_000_000_000 / 1),
            "Gmol",
            "gigamole",
            "gigamoles",
            "Ggmol",
            "Ggmole",
            "Ggmoles"
        ),
        (
            Teramole,
            ratio!(1_000_000_000_000 / 1),
            "Tmol",
            "teramole",
            "teramoles",
            "Tgmol",
            "Tgmole",
            "Tgmoles"
        ),
        (
            Millimole,
            ratio!(1 / 1_000),
            "mmol",
            "millimole",
            "millimoles",
            "mgmol",
            "mgmole",
            "mgmoles"
        ),
        (
            Micromole,
            ratio!(1 / 1_000_000),
            "umol",
            "micromole",
            "micromoles",
            "ugmol",
            "ugmole",
            "ugmoles"
        ),
        (
            Nanomole,
            ratio!(1 / 1_000_000_000),
            "nmol",
            "nanomole",
            "nanomoles",
            "ngmol",
            "ngmole",
            "ngmoles"
        ),
        (
            Picomole,
            ratio!(1 / 1_000_000_000_000),
            "pmol",
            "picomole",
            "picomoles",
            "pgmol",
            "pgmole",
            "pgmoles"
        ),
        // Non-SI units
        (
            PoundMole,
            ratio!(45_359_237 / 100_000_000),
            "lbmol",
            "pound-mole",
            "pound-moles"
        )
    ]
);
