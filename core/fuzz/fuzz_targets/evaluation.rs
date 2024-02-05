#![no_main]

use libfuzzer_sys::fuzz_target;
use siffra::evaluation::{evaluate_line, SiffraState};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let mut state = SiffraState::new();
        let _ = evaluate_line(s, &mut state);
    }
});
