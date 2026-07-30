//! Dumps the engine catalog (functions, constants, units) as JSON.
//!
//! The desktop app consumes the output as
//! `desktop/src/lib/catalog.generated.json`.
//! Regenerate with:
//!
//! ```sh
//! cargo run -p siffra --example dump_catalog \
//!     > desktop/src/lib/catalog.generated.json
//! ```

fn main() {
    let catalog = siffra::catalog::catalog();
    println!("{}", serde_json::to_string_pretty(&catalog).unwrap());
}
