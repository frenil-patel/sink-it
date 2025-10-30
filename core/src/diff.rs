//! diff.rs
//! Produce a *top-level* edit script describing how `other` changed from `base`.
//! We compare only units we collected in Step 1 (functions, classes, imports, etc.).
//!
//! Edit kinds we emit for the MVP:
//! - insert(kind,name,snippet)
//! - update(kind,name,snippet)   (same unit exists, but byte range changed)
//! - delete(kind,name)

use crate::ast::{AstFile, TopLevel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edit {
    pub kind: String,      // "insert" | "update" | "delete"
    pub unit_kind: String, // e.g. "function_declaration"
    pub name: String,      // identifier ("" if unknown)
    pub payload: Option<String>, // code snippet for insert/update
}

fn unit_key(u: &TopLevel) -> Option<(String, String)> {
    // Use (kind, name) as the semantic identity for top-level units.
    // If a unit has no name (rare at top-level), skip it for MVP.
    Some((u.kind.clone(), u.name.clone()?))
}

/// Compute edits to go from base -> other at top level.
pub fn diff_top_level(base: &AstFile, other: &AstFile) -> Vec<Edit> {
    use std::collections::{HashMap, HashSet};

    let mut base_map: HashMap<(String, String), (usize, usize)> = HashMap::new();
    for u in &base.units {
        if let Some(key) = unit_key(u) {
            base_map.insert(key, (u.start_byte, u.end_byte));
        }
    }

    let mut other_map: HashMap<(String, String), (usize, usize)> = HashMap::new();
    for u in &other.units {
        if let Some(key) = unit_key(u) {
            other_map.insert(key, (u.start_byte, u.end_byte));
        }
    }

    let mut edits = Vec::new();

    // Inserts/Updates (units present in OTHER)
    for u in &other.units {
        if let Some((kind, name)) = unit_key(u) {
            match base_map.get(&(kind.clone(), name.clone())) {
                None => {
                    // New unit inserted
                    let snippet = &other.code[u.start_byte..u.end_byte];
                    edits.push(Edit {
                        kind: "insert".into(),
                        unit_kind: kind,
                        name,
                        payload: Some(snippet.to_string()),
                    });
                }
                Some((s, e)) => {
                    // Unit existed in base; if byte range differs, call it an update (MVP)
                    if *s != u.start_byte || *e != u.end_byte {
                        let snippet = &other.code[u.start_byte..u.end_byte];
                        edits.push(Edit {
                            kind: "update".into(),
                            unit_kind: kind,
                            name,
                            payload: Some(snippet.to_string()),
                        });
                    }
                }
            }
        }
    }

    // Deletions (present in base but missing in OTHER)
    let other_keys: HashSet<_> = other_map.keys().cloned().collect();
    for u in &base.units {
        if let Some(key) = unit_key(u) {
            if !other_keys.contains(&key) {
                edits.push(Edit {
                    kind: "delete".into(),
                    unit_kind: key.0,
                    name: key.1,
                    payload: None,
                });
            }
        }
    }

    edits
}