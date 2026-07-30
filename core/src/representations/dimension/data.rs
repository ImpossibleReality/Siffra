use crate::{quantity, ratio};

// A byte is 8 bits. Decimal prefixes use powers of 1000; binary prefixes use
// powers of 1024. Both live on the same axis, so `1 GB in MiB` converts.
quantity!(
    Data,
    [
        (Bit, ratio!(1 / 1), "b", "bit", "bits"),
        (Kilobit, ratio!(1_000 / 1), "kbit", "kilobit", "kilobits", "kb", "Kb"),
        (Megabit, ratio!(1_000_000 / 1), "Mbit", "megabit", "megabits", "Mb"),
        (
            Gigabit,
            ratio!(1_000_000_000 / 1),
            "Gbit",
            "gigabit",
            "gigabits",
            "Gb"
        ),
        (
            Terabit,
            ratio!(1_000_000_000_000 / 1),
            "Tbit",
            "terabit",
            "terabits",
            "Tb"
        ),
        (Byte, ratio!(8 / 1), "B", "byte", "bytes"),
        (Kilobyte, ratio!(8_000 / 1), "kB", "kilobyte", "kilobytes", "KB"),
        (
            Megabyte,
            ratio!(8_000_000 / 1),
            "MB",
            "megabyte",
            "megabytes"
        ),
        (
            Gigabyte,
            ratio!(8_000_000_000 / 1),
            "GB",
            "gigabyte",
            "gigabytes"
        ),
        (
            Terabyte,
            ratio!(8_000_000_000_000 / 1),
            "TB",
            "terabyte",
            "terabytes"
        ),
        (
            Petabyte,
            ratio!(8_000_000_000_000_000 / 1),
            "PB",
            "petabyte",
            "petabytes"
        ),
        (Kibibyte, ratio!(8_192 / 1), "KiB", "kibibyte", "kibibytes"),
        (
            Mebibyte,
            ratio!(8_388_608 / 1),
            "MiB",
            "mebibyte",
            "mebibytes"
        ),
        (
            Gibibyte,
            ratio!(8_589_934_592 / 1),
            "GiB",
            "gibibyte",
            "gibibytes"
        ),
        (
            Tebibyte,
            ratio!(8_796_093_022_208 / 1),
            "TiB",
            "tebibyte",
            "tebibytes"
        ),
        (
            Pebibyte,
            ratio!(9_007_199_254_740_992 / 1),
            "PiB",
            "pebibyte",
            "pebibytes"
        )
    ]
);
