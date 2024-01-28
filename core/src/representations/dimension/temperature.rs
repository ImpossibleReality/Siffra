use crate::{quantity, ratio};

// NOTE: This is for temperature INTERVALS, not absolute temperatures.

quantity!(
    Temperature,
    [
        // Note; Interval between celsius is in kelvins.
        (Kelvin, ratio!(1 / 1), "K", "kelvin", "kelvins"),
        (Fahrenheit, ratio!(5 / 9), "F", "farenheit", "farenheits")
    ]
);
