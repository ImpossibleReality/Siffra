// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use siffra_desktop::evaluation::{evaluate_line, SiffraState};
use siffra_desktop::representations::Value;

#[derive(Serialize, Deserialize)]
struct SiffraLineOutput {
    line: u16,
    output: String,
}

fn display_value(val: Value) -> String {
    let (mut val, dim) = val.into_parts();

    let mut output = String::new();

    output.push_str(&val.to_string());

    let dim_string = dim.to_string();

    if !dim_string.is_empty() {
        output.push_str(" ");
        output.push_str(&dim_string);
    }

    output
}

#[tauri::command]
fn get_result(input: &str) -> Vec<SiffraLineOutput> {
    let mut state = SiffraState::new();
    let mut output = Vec::new();
    for (i, line) in input.lines().enumerate() {
        let result = evaluate_line(line, &mut state);
        match result {
            Ok(Some(value)) => {
                output.push(SiffraLineOutput {
                    line: i as u16,
                    output: display_value(value),
                });
            }
            Ok(None) => {}
            Err(_) => {
                output.push(SiffraLineOutput {
                    line: i as u16,
                    output: "Error".to_string(),
                });
            }
        }
    }
    output
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_result])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
