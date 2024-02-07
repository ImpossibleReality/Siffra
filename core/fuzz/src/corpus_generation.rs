use md5::compute as md5_compute;
use rand::Rng;

use std::fs::File;
use std::io::Write;

const OPERATIONS: [char; 5] = ['+', '-', '*', '/', '^'];
const UNITS: [&str; 6] = ["m", "s", "kg", "mol", "g", "ms"];

const CORPUS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/corpus/evaluation");

fn generate_random_number(rng: &mut impl Rng) -> String {
    let mut number = String::new();
    let num_digits = rng.gen_range(1..10);
    for _ in 0..num_digits {
        number.push((rng.gen_range(0..10) + 48) as u8 as char);
    }
    // Add decimal point
    if rng.gen_bool(0.2) {
        number.push('.');
        let num_digits = rng.gen_range(1..10);
        for _ in 0..num_digits {
            number.push((rng.gen_range(0..10) as u8 + 48) as char);
        }
    }
    number
}

fn generate_random_expr(rng: &mut impl Rng) -> String {
    let mut expr = String::new();
    let mut num_tokens = rng.gen_range(1..50);
    let mut was_last_number = false;
    let mut stack_size: u8 = 0;
    while num_tokens > 0 {
        expr.push_str(" ");
        if !was_last_number && rng.gen_bool(0.3) {
            expr.push_str("(");
            stack_size += 1;
        } else if !was_last_number {
            expr.push_str(&generate_random_number(rng));
            expr.push_str(" ");
            if rng.gen_bool(0.5) {
                expr.push_str(UNITS[rng.gen_range(0..6)]);
            }
            was_last_number = true;
        } else if was_last_number && stack_size > 0 && rng.gen_bool(0.6) {
            expr.push_str(")");
            was_last_number = true;
        } else if was_last_number {
            expr.push_str(OPERATIONS[rng.gen_range(0..5)].to_string().as_str());
            was_last_number = false;
        }
        num_tokens -= 1;
    }
    while stack_size > 0 {
        expr.push_str(")");
        stack_size -= 1;
    }
    expr
}

fn main() {
    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let expr = generate_random_expr(&mut rng);
        let hash = md5_compute(expr.as_bytes());
        let hash_str = format!("{:x}", hash);
        // Write to corpus/[hash].txt
        let mut file = File::create(format!("{}/{}.txt", CORPUS_DIR, hash_str)).unwrap();
        file.write_all(expr.as_bytes()).unwrap();
    }
}
