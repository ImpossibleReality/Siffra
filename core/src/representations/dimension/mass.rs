use crate::{quantity, ratio, si_prefixable};

// SI prefixes attach to the gram, so scaled grams come from the prefix table.
quantity!(
    Mass,
    [
        (Gram, ratio!(1 / 1), "g", "gram", "grams"),
        // Derived expansions use kilograms, so it remains an entry of its own.
        (Kilogram, ratio!(1_000 / 1), "kg", "kilogram", "kilograms"),
        (Tonne, ratio!(1_000_000 / 1), "t", "tonne", "tonnes"),
        (
            Ounce,
            ratio!(28349523125 / 1000000000),
            "oz",
            "ounce",
            "ounces"
        ),
        (Pound, ratio!(45359237 / 100000), "lb", "pound", "pounds"),
        (Stone, ratio!(635_029_318 / 100000), "st", "stone", "stones"),
        // A short ton is 2,000 pounds.
        (Ton, ratio!(90_718_474_000 / 100000), "tn", "ton", "tons"),
        // An imperial ton is 2,240 pounds.
        (
            LongTon,
            ratio!(101_604_690_880 / 100000),
            "lt",
            "long ton",
            "long tons"
        ),
        (Carat, ratio!(1 / 5), "ct", "carat", "carats"),
        (Grain, ratio!(6_479_891 / 100_000_000), "gr", "grain", "grains"),
        (
            Dram,
            ratio!(28_349_523_125 / 16_000_000_000),
            "dr",
            "dram",
            "drams"
        ),
        (
            Hundredweight,
            ratio!(45_359_237 / 1_000),
            "cwt",
            "hundredweight",
            "hundredweights"
        ),
        (Slug, ratio!(1_459_390_294 / 100_000), "slug", "slugs", "slg"),
        (
            TroyOunce,
            ratio!(311_034_768 / 10_000_000),
            "ozt",
            "troy ounce",
            "troy ounces"
        ),
        (
            TroyPound,
            ratio!(3_732_417_216 / 10_000_000),
            "lbt",
            "troy pound",
            "troy pounds"
        ),
        (
            Dalton,
            Float::parse("1.66053906660e-24").unwrap(),
            "Da",
            "dalton",
            "daltons",
            "amu",
            "u"
        )
    ]
);

// The tonne is prefixed too, because `kt` / `Mt` / `Gt` is how large masses are
// actually written.
si_prefixable!(Mass, [Gram, Tonne]);
