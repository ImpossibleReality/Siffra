//! Linear temperature intervals, distinct from absolute temperatures.

use crate::{quantity, ratio};
quantity!(
    TemperatureInterval,
    [
        // Celsius intervals use kelvin-sized units.
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
        // 1 degree Rankine is the same size as 1 degree Fahrenheit.
        (
            Rankine,
            ratio!(5 / 9),
            "iR",
            "interval of rankine",
            "interval rankine"
        )
    ]
);
