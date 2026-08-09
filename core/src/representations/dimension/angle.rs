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

si_prefixable!(Angle, [Radian]);
