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
    let (sign, string, exp) =
        val.to_sign_string_exp_round(10, Some(10), rug::float::Round::Nearest);

    if sign {
        output.push_str("-");
    }

    // remove trailing zeros
    let mut string = string.trim_end_matches('0').to_string();

    // remove leading zeros
    let mut string = string.trim_start_matches('0').to_string();

    if let Some(exp) = exp {
        if exp > 10 || exp < -10 {
            // insert decimal point after first digit if necessary
            if string.len() > 1 {
                string.insert(1, '.');
            }
            output.push_str(&string);
            output.push_str("E");
            output.push_str(&(exp - 1).to_string());
        } else {
            // insert decimal point at correct position
            if exp > 0 {
                if exp >= string.len() as i32 {
                    string.push_str(&"0".repeat((exp - string.len() as i32) as usize));
                }
                string.insert(exp as usize, '.');
                if string.ends_with('.') {
                    string.pop();
                }
            } else {
                string.insert_str(0, &"0".repeat(-exp as usize + 1));
                string.insert(1, '.');
            }

            output.push_str(&string);
        }
    } else {
        output.push_str("0");
    }

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
