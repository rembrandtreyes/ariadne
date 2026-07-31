use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds the rebuilt dashboard (dashboard-ui/, Svelte + Vite + TypeScript)
/// with bun at compile time and stages the output in OUT_DIR, where
/// src/dashboard/mod.rs embeds it via rust-embed. bun is a compile-time
/// requirement only — the produced binary has zero runtime Node/Bun
/// dependency (ratified in the dashboard-rebuild ISA, D-6).
fn main() {
    println!("cargo:rerun-if-changed=dashboard-ui/src");
    println!("cargo:rerun-if-changed=dashboard-ui/index.html");
    println!("cargo:rerun-if-changed=dashboard-ui/package.json");
    println!("cargo:rerun-if-changed=dashboard-ui/bun.lock");
    println!("cargo:rerun-if-changed=dashboard-ui/vite.config.ts");
    println!("cargo:rerun-if-changed=dashboard-ui/svelte.config.js");
    println!("cargo:rerun-if-changed=dashboard-ui/tsconfig.json");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let dist = out_dir.join("dashboard-dist");

    // docs.rs builds run in a sandbox without bun or network access; embed an
    // empty asset set there so the crate still documents.
    if std::env::var_os("DOCS_RS").is_some() {
        std::fs::create_dir_all(&dist).expect("failed to create empty dashboard-dist");
        return;
    }

    let ui_dir = Path::new("dashboard-ui");
    if !ui_dir.is_dir() {
        panic!("dashboard-ui/ directory is missing — cannot build the embedded dashboard");
    }

    if !ui_dir.join("node_modules").is_dir() {
        run_bun(ui_dir, &["install"], &dist);
    }
    run_bun(ui_dir, &["run", "build"], &dist);

    if !dist.join("index.html").is_file() {
        panic!(
            "dashboard build produced no index.html in {} — check dashboard-ui/vite.config.ts",
            dist.display()
        );
    }
}

fn run_bun(ui_dir: &Path, args: &[&str], dist: &Path) {
    let status = Command::new("bun")
        .args(args)
        .current_dir(ui_dir)
        .env("VITE_OUT_DIR", dist)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "failed to launch `bun {}`: {e}\n\
                 bun is a compile-time requirement for building ariadne's dashboard \
                 (https://bun.sh). The finished binary does not need it.",
                args.join(" ")
            )
        });
    if !status.success() {
        panic!("`bun {}` failed with {status}", args.join(" "));
    }
}
