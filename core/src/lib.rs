//! sink_core: Step 2 adds diff + compose, and exposes a 3-way merge helper.

mod ast;
mod diff;
mod compose;

pub use ast::{AstFile, TopLevel, parse_typescript_to_ast, AstLanguage};
pub use diff::{Edit, diff_top_level};
pub use compose::{MergeOutcome, compose_top_level};

use anyhow::*;

/// High-level 3-way merge helper for a single file (top-level only, MVP).
pub fn three_way_merge_top_level(
    base_code: &str,
    a_code: &str,
    b_code: &str,
    lang: AstLanguage,
) -> Result<MergeOutcome> {
    // 1) Parse
    let t0 = parse_typescript_to_ast(base_code, lang)?;
    let ta = parse_typescript_to_ast(a_code, lang)?;
    let tb = parse_typescript_to_ast(b_code, lang)?;

    // 2) Diff (Base->A and Base->B)
    let ea = diff::diff_top_level(&t0, &ta);
    let eb = diff::diff_top_level(&t0, &tb);

    // 3) Compose
    let out = compose::compose_top_level(&t0, &ea, &eb)?;
    Ok(out)
}