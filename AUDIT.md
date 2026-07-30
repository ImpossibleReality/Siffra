# Siffra accuracy & dimensionality audit — 2026-07-22

Findings from a read-through of `core/` plus empirical verification (every item marked
**confirmed** was reproduced by evaluating the shown input against the current build).
Ordered by severity. Line numbers refer to the current working tree.

## A. Confirmed bugs — silent wrong answers (fix first)

1. **`1e-60` evaluates to −57.28…** — the number parser only accepts uppercase `E`
   exponents (`core/src/parser/mod.rs:118`), so `1e-60` parses as `1 * e - 60` (Euler's
   constant via implicit multiplication). No error, wildly wrong value. Same family:
   `1E+5` fails (no `+` after `E`), `.5` fails (no leading-dot decimals) — those at
   least error. Fix: accept `e`/`E` and optional `+`; consider rejecting a bare `e`
   right after a number instead of implicitly multiplying.

2. **`min(1,2)` = 12** — the digit parser treats `,` as a thousands separator between
   any two digits (`parser/mod.rs:93-99`), so inside `f(a,b)` a comma directly between
   digits is eaten by the number: `min(1,2)` → `min(12)`, `min(1,234, 5)` →
   `min(1234, 5)`. Also `1,23` = 123 (grouping never validated as groups of 3).
   Affects every multi-arg function. Fix: validate 3-digit grouping, or don't allow
   `,` inside numbers within argument lists.

3. **Trig/log/exp ignore units entirely** — `sin(90 deg)` = sin(90 **radians**) =
   0.894; `sin(5 m)` and `ln(2 kg)` silently discard the unit. The `sin`/`cos`/`tan`/
   `asin`/`acos`/`atan`/`exp`/`log`/`ln` arms in `core/src/evaluation/mod.rs:909-1078`
   use `args[0].value` without any dimension check (contrast `sinh` at :1000 which
   checks). Fix: convert Angle-dimensioned args to radians first; error on any other
   dimension. Inverse trig should likewise decide whether to return `rad`-tagged
   results (currently unitless; `atan2` too).

4. **Stoichiometric bridge accepts mismatched dimensions** — `1 [g H2O] in [mol H2O * m]`
   succeeds (0.0555 mol·m). `Dimension::stoichiometric_ratio`
   (`core/src/representations/dimension/mod.rs:440-470`) iterates only `self`'s terms;
   terms present only in `other` (the stray `m`) are never checked. Also `bridge_term`
   (`mod.rs:491`, `:511`) uses `if *power != one && *other_power != one { continue }`,
   which lets a bridge through when only one side has power 1. Fix: require the axis
   sets to match exactly after bridging, and require both powers = 1 (or equal).

5. **`lbmol` off by 1000×** — `1 lbmol in mol` = 0.45359237; a pound-mole is
   453.59237 mol. `core/src/representations/dimension/amount.rs:100` has ratio
   `45_359_237 / 100_000_000`; should be `45_359_237 / 100_000`. (The conformance
   suite shows `lbmol … no ref` and skips it — see §D.)

6. **Molar masses computed in `f32`** — `18.01528 g H2O in mol H2O` = 1.0000155
   (should be ≈1). `Compound::particulate_mass` returns `f32`
   (`core/src/representations/dimension/chemical/compound.rs:13`), atomic masses in
   `element.rs` are `f32`, and the result is round-tripped through `to_string()` in
   `dimension/mod.rs:498`. ~7 significant digits of accuracy displayed to 30. Fix:
   store atomic masses as `Float`/string constants.

7. **Degree→radian ratio is a truncated rational** — `180 deg to rad` displays
   3.14159265358979323846264**461102**; π is …46264**338327**. The hard-coded ratio in
   `core/src/representations/dimension/angle.rs:9` is only ~22 digits accurate while
   display shows 30. Revolution/gradian/arcmin/arcsec inherit it. Fix: derive from
   `Float::pi() / 180` at full 256-bit precision (note the astro-float pow-hang
   memory: π itself is cached, this is just a division).

8. **`gamma` accurate to only ~16 digits, displayed to 30** — `gamma(0.5)` =
   1.772453850905516**58508…** vs √π = 1.772453850905516**02729…**. Lanczos
   coefficients are `f64` (`core/src/representations/float.rs:230`). Affects
   non-integer `factorial` too (`3.5!`, `(-0.5)!`). Fix: either use a
   higher-precision gamma or round gamma results to ~14 significant digits so the
   display never shows wrong digits.

9. **`round_inexact` zeroes small magnitudes** — it rounds to 50 *decimal places*
   (`float.rs:203`), so any inexact result < 5e-51 becomes 0: `exp(-200)` = 0,
   `sin(1E-60)` = 0. It's applied to nearly every function result in evaluation.
   Fix: round to N *significant* digits instead of N decimal places.

10. **Integer 0 rejected by integer-taking functions** — `round(1.234, 0)`,
    `choose(5, 0)`, `gcd(0, 6)`, `permute(5, 0)` all error. `bounded_integer`
    (`evaluation/mod.rs:27-33`) parses via `Float::to_string()`, and zero prints as
    `"0.0"` (`float.rs:500`), which fails `parse::<i32>()`. Fix: special-case zero or
    parse more robustly (large integers print with `E` notation and also fail —
    currently that's the intended out-of-range path, but it conflates the two).

## B. Confirmed bugs — errors/display (lower stakes)

11. **`2^-3` is a parse error** — the power rule (`parser/mod.rs:560-579`) descends to
    `factorial`/`product`, skipping unary minus, so a negative exponent needs
    parentheses. Same for `0^-1`.
12. **Display output is not always re-parseable** —
    `°C` is displayed but `5 °C` won't parse (aliases are `C`, `degC`, `celsius`;
    `°` isn't `is_ascii_alphabetic`). Fractional unit powers don't display:
    `(2 m)^0.5` shows `[m]`, not `m^0.5` — `Dimension::Display` only prints `^p` for
    `p > 1` (`dimension/mod.rs:321`); and bracketed unit powers only parse integers,
    so `m^0.5` can't be typed back either. Numerator powers strictly between 0 and 1
    are silently hidden — that's a wrong-looking unit, arguably A-severity.
13. **Domain errors surface as `NaN`** — `sqrt(-4)`, `asin(2)`, `log(-5)`,
    `root_3(-27)` all display `NaN` (and NaN flows into `prev`/`total`). Real odd
    roots of negatives (`(-8)^(1/3)`) are NaN because non-integer pow goes through
    `exp(y·ln x)`. Decide: proper domain errors, and optionally support odd roots.
14. **`0^0` = 1** — fine if intended; document it.

## C. Design decisions to confirm (currently surprising, maybe intended)

15. **Conversion binds tighter than `+`/`-`** — `1 h + 30 min in min` = **1.5 h**
    (the `in min` applies to `30 min` only). Users almost certainly expect 90 min.
16. **Absolute temperature semantics** — `2 C * 2` = 4 °C (stays affine because the
    dimension shape is still a lone °C, despite the "decays under any arithmetic" doc
    on `AbsoluteTemperature`); `20 C + 20 F` = 31.11 °C (rhs treated as an interval);
    `0 C + 0 C` = 0 °C. Consistent-ish, but decide and pin with tests, including
    `C - C` (arguably should yield an interval, currently °C).
17. **`K`/`k` = 1000** — `5K` = 5000 but `5 K` = 5 kelvin (`evaluation/mod.rs:371`).
    One space changes the meaning by three orders of magnitude.
18. **Percent asymmetry** — `10 + 5%` = 10.5 but `5% + 10` errors (intended per
    comment in `value.rs`); `50% * 50%` = 0.25 (unitless, not 25%). Pin with tests.
19. **Time conventions** — month = 30.4375 d, year = 365.25 d (Julian). GNU units
    uses the tropical year (conformance shows the 2.14e-5 delta as "close"). Document.
20. **Assignments can shadow anything** — `pi = 3` shadows the constant; `in = 5`
    assigns a variable that can never be read (reserved on the read path only,
    `parser/mod.rs:692`).
21. **Comment lines reset the running `total`** (`evaluation/mod.rs:204`) — intended
    block-delimiter behavior? Also: once one line's units mismatch the total, the
    total is dead (`None`) until the next comment.
22. **256-bit precision ceiling** — `10^100 + 1 - 10^100` = 0; `1000!` is rounded
    despite the "exact product" path (2568 digits > ~77-digit precision). Fine for a
    display calculator; document the boundary.
23. **Currency**: `to_float` falls back to `Float::from(0)` on a parse failure
    (`currency.rs:231`) → `factor()` = 1/0 = Inf rather than "no rate". Unlikely but
    cheap to fix.

## D. Testing recommendations (specific, for the fixing session)

- **Fix the conformance gap first**: `cargo run -p conformance` currently reports
  `lbmol … no ref` and counts it as success. Make "no reference" a hard failure (or
  give such units hand-written expected factors). That single change would have
  caught bug 5; re-run after every unit-table edit.
- **Display round-trip property**: for every catalog unit alias U, evaluate `1 U`,
  print value+dimension, re-parse the printed form, and assert equality. Catches
  bug 12 (°C, fractional powers) and any future display/parse drift.
- **Conversion involution property**: for random values and same-axis unit pairs
  (A,B): `x A in B in A ≈ x A` at ≥ 30 significant digits; plus triangle consistency
  `(A→B→C) ≈ (A→C)`. Include the affine temperatures at anchor points (−40, 0, 32,
  100, 273.15) and the mass↔mole bridge with several compounds.
- **Dimensional soundness fuzz**: generate random `[unit-product] in [unit-product]`
  pairs; assert a conversion succeeds *iff* dimensionalities match (or a legal
  stoichiometric bridge). This is the systematic version of bug 4.
- **Angle-aware trig table** (after fixing bug 3): `sin(90 deg)=1`, `sin(pi rad)≈0`,
  `sin(100 grad)=1`, `cos(1 rev)=1`, plus `sin(5 m)` → error.
- **30-digit reference values**: for each function (trig, ln, log2, gamma, roots,
  hypot, atan2, financial), compare a handful of inputs against mpmath/Wolfram values
  at 35 digits, asserting all 30 displayed digits match. Catches bugs 7 and 8 and any
  future `round_inexact` regressions; include tiny/huge magnitudes (`exp(-200)`,
  `sin(1e-60)`, `2^0.5 * 1e60`) for bug 9.
- **Parser misparse suite** (silent-wrong-answer guard): `1e5`, `1e-60`, `min(1,2)`,
  `1,23`, `5K` vs `5 K`, `100 in km`, `x in y` for units named like keywords, `2^-3`.
  Assert either the correct value or an error — never a different valid-looking value.
- **Percent operator matrix**: {`%`, `on`, `off`, `of`, `as a % of/on/off`,
  `of/on/off what is`} × {unitless, USD, m} × {0, negative, >100%} — the zero/negative
  edges (`0% of what is 5`) and unit-carrying bases are untested today.
- **Financial functions**: `apy`/`apr` round-trip (`apr(apy(r,n),n) ≈ r`), `pmt` vs a
  known amortization table, negative rates near −100%, `rate=0` paths.
- **Currency**: with `install_static_rates`, test cross-rate math (USD→JPY via EUR),
  missing-rate errors, `USD^2` display, and `$` prefix parsing with arithmetic.
- **Block total/state**: unit-mismatch poisoning, comment reset, `prev` after an
  error line, variable shadowing of `pi`/`k`.

## E. Verified-good (no action, keep covered)

Affine temperature conversion anchors (`-40 F to C` = −40, `98.6 F to C` = 37,
`300 K to C` = 26.85); `mph*s` → miles collapse; `N·m` vs `J` display distinction;
`kg·m/s² in N` collapse; binary/decimal data units (`1 GB in MiB` = 953.674…);
floor-mod semantics (`mod(-7,3)` = 2); display rounding of binary artifacts
(`0.1+0.2` = 0.3, trailing-nine carry, `2E-77` → 0); comma grouping in *output*;
`1 [1/s] in Hz`; `%`↔scalar conversions; exact integer factorials ≤ ~77 digits.
