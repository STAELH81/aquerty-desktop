//! Generate Aquerty Stop license keys.
//!
//! Usage:
//!   cargo run --bin gen-license -- lifetime ORDER123
//!   cargo run --bin gen-license -- annual ORDER123
//!   cargo run --bin gen-license -- annual ORDER123 20270814
//!   cargo run --bin gen-license -- batch-lifetime 50 prefix
//!
//! Run from src-tauri: cargo run --bin gen-license -- …

fn main() {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_help();
        std::process::exit(1);
    }

    let cmd = args.remove(0).to_lowercase();
    match cmd.as_str() {
        "lifetime" | "life" => {
            let id = args.first().map(String::as_str).unwrap_or("MANUAL");
            println!("{}", aquerty_stop_lib::license_generate_lifetime(id));
        }
        "annual" | "yr" => {
            let id = args.first().map(String::as_str).unwrap_or("MANUAL");
            let exp = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(aquerty_stop_lib::license_default_annual_expiry);
            println!("{}", aquerty_stop_lib::license_generate_annual(id, exp));
        }
        "batch-lifetime" => {
            let n: usize = args
                .first()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50);
            let prefix = args.get(1).map(String::as_str).unwrap_or("GUM");
            for i in 1..=n {
                let id = format!("{prefix}{i:04}");
                println!("{}", aquerty_stop_lib::license_generate_lifetime(&id));
            }
        }
        "help" | "-h" | "--help" => print_help(),
        other => {
            eprintln!("Unknown command: {other}");
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_help() {
    eprintln!(
        "\
Aquerty Stop license generator

  cargo run --bin gen-license -- lifetime <ID>
  cargo run --bin gen-license -- annual <ID> [YYYYMMDD]
  cargo run --bin gen-license -- batch-lifetime [count] [prefix]

Examples:
  cargo run --bin gen-license -- lifetime BUYER42
  cargo run --bin gen-license -- annual BUYER42 20270814
  cargo run --bin gen-license -- batch-lifetime 50 GUM
"
    );
}
