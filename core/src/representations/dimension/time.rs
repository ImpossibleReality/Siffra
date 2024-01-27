use crate::{quantity, ratio};

quantity!(
    Time,
    [
        // SI units
        (
            Second,
            ratio!(1 / 1),
            "s",
            "second",
            "sec",
            "seconds",
            "secs"
        ),
        (
            Millisecond,
            ratio!(1 / 1000),
            "ms",
            "millisecond",
            "milliseconds",
            "millisec",
            "msec",
            "msecs"
        ),
        (
            Microsecond,
            ratio!(1 / 1000000),
            "us",
            "microsecond",
            "microseconds",
            "microsec",
            "usec",
            "usecs"
        ),
        (
            Nanosecond,
            ratio!(1 / 1000000000),
            "ns",
            "nanosecond",
            "nanoseconds",
            "nanosec",
            "nsec",
            "nsecs"
        ),
        (
            Picosecond,
            ratio!(1 / 1000000000000),
            "ps",
            "picosecond",
            "picoseconds",
            "picosec",
            "psec",
            "psecs"
        ),
        (Minute, ratio!(60 / 1), "min", "minute", "minutes"),
        (Hour, ratio!(3600 / 1), "h", "hour", "hours"),
        (Day, ratio!(86400 / 1), "d", "day", "days"),
        (Week, ratio!(604800 / 1), "w", "week", "weeks"),
        (Month, ratio!(2629800 / 1), "mo", "month", "months"),
        (Year, ratio!(31557600 / 1), "y", "year", "years")
    ]
);
