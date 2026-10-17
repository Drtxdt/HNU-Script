//! Inspect the bundled catalogue with the same decoder used by grading.
fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&sql_killer_core::exercises::catalog(0)).unwrap()
    );
}
