use crate::{quantity, ratio, si_prefixable};

// Sub-second scales come from the SI prefix table. Larger units stay unprefixed
// so names such as `d`, `h`, and `y` remain unambiguous.
quantity!(
    Time,
    [
        (
            Second,
            ratio!(1 / 1),
            "s",
            "second",
            "sec",
            "seconds",
            "secs"
        ),
        (Minute, ratio!(60 / 1), "min", "minute", "minutes"),
        (Hour, ratio!(3600 / 1), "h", "hour", "hours"),
        (Day, ratio!(86400 / 1), "d", "day", "days"),
        (Week, ratio!(604800 / 1), "w", "week", "weeks"),
        (
            Fortnight,
            ratio!(1209600 / 1),
            "fortnight",
            "fortnights",
            "fn"
        ),
        (Month, ratio!(2629800 / 1), "mo", "month", "months"),
        (Year, ratio!(31557600 / 1), "y", "year", "yr", "years"),
        (Decade, ratio!(315576000 / 1), "decade", "decades", "dec"),
        (
            Century,
            ratio!(3155760000 / 1),
            "century",
            "centuries",
            "cent"
        ),
        (
            Millennium,
            ratio!(31557600000 / 1),
            "millennium",
            "millennia",
            "kyr"
        )
    ]
);

// `sec` is an abbreviation rather than a spelled-out name, so a *symbol* prefix
// may sit on it: `msec` and `usec` read like `ms` and `us`.
si_prefixable!(Time, [Second ["sec", "secs"]]);
