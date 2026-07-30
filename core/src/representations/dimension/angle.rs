use crate::{quantity, ratio, si_prefixable};

quantity!(
    Angle,
    [
        (Radian, ratio!(1 / 1), "rad", "radian", "radians", "rads"),
        (
            Degree,
            &Float::pi() / &Float::from(180),
            "deg",
            "degree",
            "degs",
            "degrees"
        ),
        // Define a full turn from degrees so it stays exactly consistent with
        // them.
        (
            Revolution,
            &Angle::Degree.ratio() * &Float::from(360),
            "rev",
            "revolution",
            "revs",
            "revolutions",
            "turn",
            "turns"
        ),
        // 400 gradians in a full turn.
        (
            Gradian,
            &Angle::Revolution.ratio() / &Float::from(400),
            "grad",
            "gradian",
            "gradians",
            "gon"
        ),
        (
            ArcMinute,
            &Angle::Degree.ratio() / &Float::from(60),
            "arcmin",
            "arcminute",
            "arcminutes"
        ),
        (
            ArcSecond,
            &Angle::Degree.ratio() / &Float::from(3600),
            "arcsec",
            "arcsecond",
            "arcseconds"
        )
    ]
);

// The radian is the SI unit here, so `mrad` and friends come from the prefix
// table; degrees and their sexagesimal subdivisions are not prefixed.
si_prefixable!(Angle, [Radian]);
