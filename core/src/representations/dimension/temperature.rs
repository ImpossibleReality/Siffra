use crate::{quantity, ratio};
quantity!(
    TemperatureInterval,
    [
        (
            Kelvin,
            ratio!(1 / 1),
            "iK",
            "interval of kelvin",
            "interval of kelvins",
            "interval kelvin"
        ),
        (
            Fahrenheit,
            ratio!(5 / 9),
            "iF",
            "interval of farenheit",
            "interval farenheit"
        ),
        (
            Rankine,
            ratio!(5 / 9),
            "iR",
            "interval of rankine",
            "interval rankine"
        )
    ]
);
