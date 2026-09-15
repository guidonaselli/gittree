use std::path::PathBuf;
use repo_state::fixtures::{
    generate_dirty_tree, generate_large_history, generate_submodule_superproject,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: fixture-gen <kind> <target-dir> [options]");
        eprintln!("  kinds: superproject [count=25]");
        eprintln!("         history [commits=100000] [branches=6]");
        eprintln!("         dirty-tree [files=10000]");
        std::process::exit(1);
    }

    let kind = &args[1];
    let path = PathBuf::from(&args[2]);

    match kind.as_str() {
        "superproject" => {
            let count: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(25);
            println!("Generating {count}-submodule superproject at {}...", path.display());
            if let Err(err) = generate_submodule_superproject(&path, count) {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
        "history" => {
            let commits: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100_000);
            let branches: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(6);
            println!("Generating {commits} commits across {branches} branches at {}...", path.display());
            if let Err(err) = generate_large_history(&path, commits, branches) {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
        "dirty-tree" => {
            let files: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10_000);
            println!("Generating {files}-file dirty tree at {}...", path.display());
            if let Err(err) = generate_dirty_tree(&path, files) {
                eprintln!("Error: {err}");
                std::process::exit(1);
            }
        }
        other => {
            eprintln!("Unknown fixture kind: {other}");
            std::process::exit(1);
        }
    }
    println!("Done.");
}
