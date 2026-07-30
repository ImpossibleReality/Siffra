use crate::{quantity, ratio, si_prefixable};

// Decimal multiples of the meter resolve from the SI prefix table.
quantity!(
    Length,
    [
        (Meter, ratio!(1 / 1), "m", "meter", "meters"),
        // Kilometer remains an entry because `kph` expands to it by name.
        (
            Kilometer,
            ratio!(1_000 / 1),
            "km",
            "kilometer",
            "kilometers"
        ),
        (Inch, ratio!(254 / 10000), "in", "inch", "inches"),
        (Foot, ratio!(3048 / 10000), "ft", "foot", "feet"),
        (Yard, ratio!(9144 / 10000), "yd", "yard", "yards"),
        (Mile, ratio!(1609344 / 1000), "mi", "mile", "miles"),
        (
            NauticalMile,
            ratio!(1852 / 1),
            "nmi",
            "nautical mile",
            "nautical miles"
        ),
        (
            Angstrom,
            ratio!(1 / 10_000_000_000),
            "angstrom",
            "ångström",
            "angstroms"
        ),
        (
            AstronomicalUnit,
            ratio!(149_597_870_700 / 1),
            "au",
            "astronomical unit",
            "astronomical units",
            "AU"
        ),
        (
            LightYear,
            ratio!(9_460_730_472_580_800 / 1),
            "ly",
            "light-year",
            "light years",
            "lightyear",
            "lightyears"
        ),
        (
            Parsec,
            ratio!(30_856_775_814_913_673 / 1),
            "pc",
            "parsec",
            "parsecs"
        ),
        (Thou, ratio!(254 / 10_000_000), "thou", "thousandth", "mil"),
        (
            Furlong,
            ratio!(201_168 / 1_000),
            "fur",
            "furlong",
            "furlongs"
        ),
        (Fathom, ratio!(18_288 / 10_000), "ftm", "fathom", "fathoms")
    ]
);

si_prefixable!(Length, [Meter]);
