//! sinkit: minimal multi-file semantic merge runner for TS/TSX repos.
//!
//! Usage:
//!   cargo run --bin sinkit -- <repo_path> <A_ref> <B_ref>
//
//! Output:
//!   Writes merged files to ./ .codesync/<original/path>.ts
//!   Prints summary of autos / conflicts.

use std::env;
use std::fs;

use std::path::PathBuf;
use std::process::Command;

use sink_core::{three_way_merge_top_level, AstLanguage};

fn main() -> anyhow::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("Usage: sinkit <repo_path> <A_ref> <B_ref>");
        std::process::exit(1);
    }
    let repo = PathBuf::from(&args[0]);
    let a_ref = &args[1];
    let b_ref = &args[2];

    // 1) merge-base
    let base_ref = git(&repo, &["merge-base", a_ref, b_ref])?;
    let base_ref = base_ref.trim().to_string();

    // 2) list files (.ts/.tsx) at base
    let files_raw = git(&repo, &["ls-tree", "-r", "--name-only", &base_ref])?;
    let files = files_raw
        .lines()
        .filter(|f| f.ends_with(".ts") || f.ends_with(".tsx"))
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    // prep output dir
    let out_root = PathBuf::from(".codesync");
    fs::create_dir_all(&out_root)?;

    let mut autos = 0usize;
    let mut conflicts = 0usize;
    let mut skipped = 0usize;

    for file in files {
        // read file content from each ref; skip if not present in A or B.
        let base_code = match git_show(&repo, &base_ref, &file) {
            Ok(s) => s,
            Err(_) => { skipped += 1; continue; }
        };
        let a_code = match git_show(&repo, a_ref, &file) {
            Ok(s) => s,
            Err(_) => { skipped += 1; continue; }
        };
        let b_code = match git_show(&repo, b_ref, &file) {
            Ok(s) => s,
            Err(_) => { skipped += 1; continue; }
        };

        // merge (treat as TS; TSX also OK since we don’t JSX-detect here)
        let res = three_way_merge_top_level(&base_code, &a_code, &b_code, AstLanguage::TypeScript)?;

        // ensure target path exists
        let out_path = out_root.join(file.replace('/', "__"));
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&out_path, res.merged_code.as_bytes())?;

        if res.conflicts.is_empty() {
            autos += 1;
            println!("✓ {}", out_path.display());
        } else {
            conflicts += 1;
            println!("⚠ {} ({} conflicts)", out_path.display(), res.conflicts.len());
            // Optionally: write a .CONFLICTS.txt with reasons
            let mut txt = String::new();
            for c in res.conflicts {
                txt.push_str("- ");
                txt.push_str(&c);
                txt.push('\n');
            }
            let mut cpath = out_path.clone();
            cpath.set_extension("conflicts.txt");
            fs::write(cpath, txt.as_bytes())?;
        }
    }

    // summary
    println!("\n--- Summary ---");
    println!("Auto-merged files: {}", autos);
    println!("With conflicts:    {}", conflicts);
    println!("Skipped (missing): {}", skipped);

    Ok(())
}

fn git(repo: &PathBuf, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("git {:?} failed: {}", args, err);
    }
}

fn git_show(repo: &PathBuf, r: &str, path: &str) -> anyhow::Result<String> {
    git(repo, &["show", &format!("{}:{}", r, path)])
}