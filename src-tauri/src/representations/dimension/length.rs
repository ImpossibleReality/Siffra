use crate::{quantity, ratio};

quantity!(
    Length,
    [
        (Meter, ratio!(1 / 1), "m", "meter", "meters"),
        (
            Kilometer,
            ratio!(1_000 / 1),
            "km",
            "kilometer",
            "kilometers"
        ),
        (
            Megameter,
            ratio!(1_000_000 / 1),
            "Mm",
            "megameter",
            "megameters"
        ),
        (
            Gigameter,
            ratio!(1_000_000_000 / 1),
            "Gm",
            "gigameter",
            "gigameters"
        ),
        (
            Terameter,
            ratio!(1_000_000_000_000 / 1),
            "Tm",
            "terameter",
            "terameters"
        ),
        (
            Petameter,
            ratio!(1_000_000_000_000_000 / 1),
            "Pm",
            "petameter",
            "petameters"
        ),
        (
            Exameter,
            ratio!(1_000_000_000_000_000_000 / 1),
            "Em",
            "exameter",
            "exameters"
        ),
        (
            Centimeter,
            ratio!(1 / 100),
            "cm",
            "centimeter",
            "centimeters"
        ),
        (
            Millimeter,
            ratio!(1 / 1_000),
            "mm",
            "millimeter",
            "millimeters"
        ),
        (
            Micrometer,
            ratio!(1 / 1_000_000),
            "um",
            "micrometer",
            "micrometers"
        ),
        (
            Nanometer,
            ratio!(1 / 1_000_000_000),
            "nm",
            "nanometer",
            "nanometers"
        ),
        (
            Picometer,
            ratio!(1 / 1_000_000_000_000),
            "pm",
            "picometer",
            "picometers"
        ),
        (
            Femtometer,
            ratio!(1 / 1_000_000_000_000_000),
            "fm",
            "femtometer",
            "femtometers"
        ),
        (
            Attometer,
            ratio!(1 / 1_000_000_000_000_000_000),
            "am",
            "attometer",
            "attometers"
        ),
        (Inch, ratio!(254 / 10000), "in", "inch", "inches"),
        (Foot, ratio!(3048 / 10000), "ft", "foot", "feet"),
        (Yard, ratio!(9144 / 10000), "yd", "yard", "yards"),
        (Mile, ratio!(1609344 / 10000), "mi", "mile", "miles"),
        (
            NauticalMile,
            ratio!(1852 / 1),
            "nmi",
            "nautical mile",
            "nautical miles"
        )
    ]
);
