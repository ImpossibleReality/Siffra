use crate::{quantity, ratio};

quantity!(
    Mass,
    [
        // SI units
        (Gram, ratio!(1 / 1), "g", "gram", "grams"),
        (Kilogram, ratio!(1000 / 1), "kg", "kilogram", "kilograms"),
        (Tonne, ratio!(1_000_000 / 1), "t", "tonne", "tonnes"),
        (Milligram, ratio!(1 / 1000), "mg", "milligram", "milligrams"),
        (
            Microgram,
            ratio!(1 / 1000000),
            "ug",
            "microgram",
            "micrograms"
        ),
        (
            Nanogram,
            ratio!(1 / 1000000000),
            "ng",
            "nanogram",
            "nanograms"
        ),
        (
            Picogram,
            ratio!(1 / 1000000000000),
            "pg",
            "picogram",
            "picograms"
        ),
        // Imperial units
        (
            Ounce,
            ratio!(28349523125 / 1000000000),
            "oz",
            "ounce",
            "ounces"
        ),
        (Pound, ratio!(45359237 / 100000), "lb", "pound", "pounds"),
        (Stone, ratio!(635_029_318 / 100000), "st", "stone", "stones"),
        // 2000 pounds (ie short ton)
        (Ton, ratio!(90_718_474_000 / 100000), "tn", "ton", "tons")
    ]
);
