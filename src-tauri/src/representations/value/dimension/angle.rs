use crate::{quantity, ratio};

quantity!(
    Angle,
    [
        (Radian, ratio!(1 / 1), "rad", "radian", "radians", "rads"),
        (
            Degree,
            ratio!(1783366216531 / 102179357533440),
            "deg",
            "degree",
            "degs",
            "degrees"
        ),
        (
            Revolution,
            ratio!(1783366216531 / 567663097408),
            "rev",
            "revolution",
            "revs",
            "revolutions"
        )
    ]
);
