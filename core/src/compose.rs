//! compose.rs
//! Compose Base->A and Base->B edit scripts *safely* at the top level, with
//! precise splicing and a small reconcilation for function parameter renames.

use anyhow::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::ast::AstFile;
use crate::diff::Edit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub merged_code: String,
    pub conflicts: Vec<String>,
}

/// Map (kind,name) -> (start,end) from Base
fn index_base_ranges(base: &AstFile) -> HashMap<(String, String), (usize, usize)> {
    let mut idx = HashMap::new();
    for u in &base.units {
        if let (kind, Some(name)) = (u.kind.clone(), u.name.clone()) {
            idx.insert((kind, name), (u.start_byte, u.end_byte));
        }
    }
    idx
}

pub fn compose_top_level(base: &AstFile, ea: &[Edit], eb: &[Edit]) -> Result<MergeOutcome> {
    let mut code = base.code.clone();
    let mut conflicts = Vec::new();

    // Collect edits
    let mut inserts: HashSet<(String, String, String)> = HashSet::new(); // (kind,name,payload)
    let mut updates_by_side: HashMap<(String, String), (Option<String>, Option<String>)> = HashMap::new(); // (kind,name) -> (A?, B?)
    let mut deletes: HashSet<(String, String)> = HashSet::new();

    let mut ingest = |edits: &[Edit], is_a: bool| {
        for e in edits {
            match e.kind.as_str() {
                "insert" => {
                    if let Some(p) = &e.payload {
                        inserts.insert((e.unit_kind.clone(), e.name.clone(), p.clone()));
                    }
                }
                "update" => {
                    let entry = updates_by_side
                        .entry((e.unit_kind.clone(), e.name.clone()))
                        .or_insert((None, None));
                    if let Some(p) = &e.payload {
                        if is_a { entry.0 = Some(p.clone()); } else { entry.1 = Some(p.clone()); }
                    }
                }
                "delete" => { deletes.insert((e.unit_kind.clone(), e.name.clone())); }
                _ => {}
            }
        }
    };
    ingest(ea, true);
    ingest(eb, false);

    // 1) delete vs update => conflict
    for key in &deletes {
        if let Some((pa, pb)) = updates_by_side.get(key) {
            if pa.is_some() || pb.is_some() {
                conflicts.push(format!("Deletion vs update on {}::{}", key.0, key.1));
            }
        }
    }

    // base ranges for splicing
    let base_idx = index_base_ranges(base);

    #[derive(Clone)]
    struct Patch { start: usize, end: usize, replacement: String }
    let mut patches: Vec<Patch> = Vec::new();

    // 2) updates (with rename-aware reconcile for functions)
    for (key, (pa, pb)) in &updates_by_side {
        if deletes.contains(key) { continue; }
        match (pa, pb) {
            (Some(a_payload), Some(b_payload)) => {
                if a_payload == b_payload {
                    // identical update
                    if let Some((s, e)) = base_idx.get(key) {
                        patches.push(Patch { start: *s, end: *e, replacement: a_payload.clone() });
                    }
                } else if key.0 == "function_declaration" {
                    if let Some(reconciled) = try_reconcile_param_rename(a_payload, b_payload) {
                        if let Some((s, e)) = base_idx.get(key) {
                            patches.push(Patch { start: *s, end: *e, replacement: reconciled });
                        }
                    } else {
                        conflicts.push(format!("Both branches updated {}::{} differently", key.0, key.1));
                    }
                } else {
                    conflicts.push(format!("Both branches updated {}::{} differently", key.0, key.1));
                }
            }
            (Some(only), None) | (None, Some(only)) => {
                if let Some((s, e)) = base_idx.get(key) {
                    patches.push(Patch { start: *s, end: *e, replacement: only.clone() });
                }
            }
            (None, None) => {}
        }
    }

    // 3) apply patches (right→left)
    patches.sort_by(|a, b| b.start.cmp(&a.start));
    for p in patches {
        if p.start <= p.end && p.end <= code.len() {
            code.replace_range(p.start..p.end, &p.replacement);
        } else {
            conflicts.push("Internal splice range out of bounds".to_string());
        }
    }

    // 4) Append inserts (MVP)
    for (_k, _n, payload) in &inserts {
        code.push_str("\n\n");
        code.push_str(payload);
        code.push('\n');
    }

    // 5) IMPORT UNION: pull all import lines from (a) current code and (b) inserted payloads,
    //    de-dupe, and place them at the very top of the file.

    // (a) collect imports from the full merged code
    let mut all_imports: Vec<String> = Vec::new();
    for line in code.lines() {
        let l = line.trim();
        if l.starts_with("import ") {
            all_imports.push(line.to_string());
        }
    }
    // (b) also scan inserted payloads in case they contain imports that weren’t captured
    for (_k, _n, payload) in inserts {
        for line in payload.lines() {
            let l = line.trim();
            if l.starts_with("import ") {
                all_imports.push(line.to_string());
            }
        }
    }
    // de-dupe while preserving order
    let mut seen = HashSet::new();
    let mut unique_imports: Vec<String> = Vec::new();
    for imp in all_imports {
        if seen.insert(imp.trim().to_string()) {
            unique_imports.push(imp);
        }
    }

    // remove all existing import lines from the body
    let mut body_lines: Vec<&str> = Vec::new();
    for line in code.lines() {
        if !line.trim().starts_with("import ") {
            body_lines.push(line);
        }
    }
    let body = body_lines.join("\n");

    // stitch: imports block (sorted for stability), then a blank line, then body (trimmed)
    unique_imports.sort(); // optional: stable order
    let imports_block = if unique_imports.is_empty() {
        String::new()
    } else {
        unique_imports.join("\n") + "\n"
    };
    let trimmed_body = body.trim_start_matches('\n').to_string();
    code = format!("{imports}\n{body}", imports = imports_block, body = trimmed_body);

    Ok(MergeOutcome { merged_code: code, conflicts })
}

/// Very small heuristic: if both payloads look like the *same* function but the
/// first parameter identifier differs, rewrite B to use A's param name and return it.
/// This lets us keep B's body edits (e.g., punctuation) while adopting A's rename.
///
/// Caveats: This is intentionally simple for an MVP.
fn try_reconcile_param_rename(a: &str, b: &str) -> Option<String> {
    // extract "function <name>(<param>..."  from both
    let (a_name, a_param) = parse_fn_name_and_first_param(a)?;
    let (b_name, b_param) = parse_fn_name_and_first_param(b)?;
    if a_name != b_name { return None; }          // not the same function
    if a_param == b_param { return None; }        // no rename; real conflict
    // replace b_param with a_param in b's payload (whole-word occurrences)
    Some(replace_ident_whole_word(b, &b_param, &a_param))
}

fn parse_fn_name_and_first_param(code: &str) -> Option<(String, String)> {
    // crude but effective: function NAME ( PARAM :
    // works for: export function NAME(param: Type) { ... }
    let code_no_export = code.replacen("export ", "", 1);
    let src = code_no_export.as_str();
    let fn_pos = src.find("function ")? + "function ".len();
    // read name
    let rest = &src[fn_pos..];
    let paren = rest.find('(')?;
    let name = rest[..paren].trim().to_string();
    // parameter segment up to ':' or ',' or ')'
    let after_paren = &rest[paren+1..];
    let end = after_paren.find(|c: char| c == ':' || c == ',' || c == ')' ).unwrap_or(after_paren.len());
    let param = after_paren[..end].trim().to_string();
    if name.is_empty() || param.is_empty() { return None; }
    Some((name, param))
}

fn replace_ident_whole_word(haystack: &str, from: &str, to: &str) -> String {
    // very small whole-word replacement (ASCII word chars)
    // to avoid messing with substrings like 'username'
    let mut out = String::with_capacity(haystack.len());
    let bytes = haystack.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + from.len() <= bytes.len() && &haystack[i..i+from.len()] == from {
            let before = if i == 0 { None } else { Some(bytes[i-1] as char) };
            let after = if i + from.len() >= bytes.len() { None } else { Some(bytes[i+from.len()] as char) };
            let is_word_left  = before.map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
            let is_word_right = after.map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false);
            if !is_word_left && !is_word_right {
                out.push_str(to);
                i += from.len();
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}