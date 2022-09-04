/*!
 * Parsing and printing of raw ir strings
 */

use num::Integer;
use std::fmt::Write;

/// Convert a Vec<u32> to raw IR string
pub fn print_to_string(ir: &[u32]) -> String {
    let mut s = String::new();

    ir.iter().enumerate().for_each(|(i, v)| {
        write!(
            s,
            "{}{}{}",
            if i == 0 { "" } else { " " },
            if i.is_even() { "+" } else { "-" },
            v
        )
        .unwrap()
    });

    s
}

#[test]
fn print_test() {
    assert_eq!(print_to_string(&[100, 50, 75]), "+100 -50 +75");
}
