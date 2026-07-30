> tldr; conventional commits, supervised AI OK, run tests

# Contributing to Siffra

Contributions are welcome — bug reports, unit definitions, parser fixes, UI polish, docs.

## Issues

**Open an issue before a pull request**, excluding trivial changes like typos.

Search first, check you're on the latest release, then include:

- If relevant, **the exact lines you typed** — Siffra is a calculator, so paste the literal input
  (`5 kg * 3 m/s^2 in N`), the result you got, and the result you expected.
- **OS, Siffra version**, and whether it's a prebuilt binary or a source build.
- For a wrong unit or conversion: **the value you believe is correct and where that reference comes
  from.**

For feature requests, walk through intended use of a feature. Keep in mind that Siffra is
lightweight, and by design shouldn't necessarily support every possible feature.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/). Release notes are curated for
each release, so write the subject line for someone reading them.

```
✅ fix(parser): treat `in` as a unit when it follows a number
❌ fix: bug
```

Types that land in the changelog: `feat`, `fix`, `perf`, `refactor`, `docs`, `build`, `ci`, `test`,
`chore`. Breaking changes take a `!` (`feat!: …`).

## Tests

```bash
cargo test --workspace
```

Bug fixes get a regression test that fails without the fix.

**If you touched the unit catalog**, also run `cargo run -p conformance -- --fails-only`, which
cross-checks factors against GNU `units` (`brew install gnu-units` on macOS). The
`cargo test -p conformance` guard silently skips when `units` isn't installed, so a green test run
proves nothing here.

## AI

AI-generated code is completely fine, and needs no disclosure. However, unsupervised AI code is not,
and honestly just bogs down actual work.

This matters a lot especially for unit factors and conversion logic. Make sure to double-check
conversion factors (a lot of this is already handled by the `conformance` crate)

## License

By contributing, you agree that your contributions will be licensed under the
[MIT License](LICENSE.md).
