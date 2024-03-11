use crate::{quantity, ratio};

// NOTE: This is for temperature INTERVALS, not absolute temperatures.
quantity!(
    TemperatureInterval,
    [
        // Note: Interval between celsius is in kelvins.
        (
            KelvinInterval,
            ratio!(1 / 1),
            "iK",
            "interval of kelvin",
            "interval of kelvins",
            "interval kelvin"
        ),
        (
            FahrenheitInterval,
            ratio!(5 / 9),
            "iF",
            "interval of farenheit",
            "interval farenheit"
        )
    ]
);
