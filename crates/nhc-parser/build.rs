//! Build script — concatenates common.pest with each product-specific
//! *_rules.pest file to produce the final grammars consumed by pest_derive.

use std::fs;
use std::path::Path;

const PRODUCTS: &[&str] = &["fstadv", "discus", "public", "wndprb"];

fn main() {
    let grammar_dir = Path::new("grammars");
    let common = fs::read_to_string(grammar_dir.join("common.pest"))
        .expect("failed to read common.pest");

    for product in PRODUCTS {
        let rules_file = grammar_dir.join(format!("{product}_rules.pest"));
        let rules = fs::read_to_string(&rules_file)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", rules_file.display()));

        let combined = format!("{common}\n{rules}");
        let out_file = grammar_dir.join(format!("{product}.pest"));
        fs::write(&out_file, combined)
            .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_file.display()));
    }

    // Re-run if any source grammar changes
    println!("cargo:rerun-if-changed=grammars/common.pest");
    for product in PRODUCTS {
        println!("cargo:rerun-if-changed=grammars/{product}_rules.pest");
    }
}
