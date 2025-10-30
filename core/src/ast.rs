//! Minimal AST representation (top-level only) for TypeScript/TSX.
//! Uses tree-sitter to parse source and collect top-level units.

use anyhow::*;
use serde::{Deserialize, Serialize};
use tree_sitter::{Language, Node, Parser, Tree};
use tree_sitter_typescript::language_tsx;
use tree_sitter_typescript::language_typescript;

#[derive(Debug, Clone, Copy)]
pub enum AstLanguage {
    TypeScript,
    Tsx,
}

fn ts_language(lang: AstLanguage) -> Language {
    match lang {
        AstLanguage::TypeScript => language_typescript(),
        AstLanguage::Tsx => language_tsx(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLevel {
    pub kind: String,         // e.g., "function_declaration"
    pub name: Option<String>, // e.g., "updateUser"
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstFile {
    pub code: String,
    pub units: Vec<TopLevel>,
}

pub fn parse_typescript_to_ast(code: &str, lang: AstLanguage) -> Result<AstFile> {
    let mut parser = Parser::new();
    parser
        .set_language(ts_language(lang))
        .map_err(|_| anyhow!("failed to set TypeScript language"))?;

    let tree = parser
        .parse(code, None)
        .ok_or_else(|| anyhow!("tree-sitter parse returned None"))?;

    let units = collect_top_level(&tree, code);

    Ok(AstFile {
        code: code.to_string(),
        units,
    })
}

fn collect_top_level(tree: &Tree, code: &str) -> Vec<TopLevel> {
    let root = tree.root_node();
    let mut out = Vec::new();

    for i in 0..root.child_count() {
        if let Some(ch) = root.child(i) {
            let kind = ch.kind();

            let mut push_unit = |n: Node| {
                let k = n.kind().to_string();
                let name = extract_unit_name(&n, code);  // <-- updated line
                out.push(TopLevel {
                    kind: k,
                    name,
                    start_byte: n.start_byte(),
                    end_byte: n.end_byte(),
                });
            };

            match kind {
                "function_declaration"
                | "class_declaration"
                | "lexical_declaration"
                | "variable_declaration"
                | "import_statement"
                | "method_definition" => {
                    push_unit(ch);
                }

                "export_statement" => {
                    let mut found_inner = false;
                    for j in 0..ch.child_count() {
                        if let Some(inner) = ch.child(j) {
                            match inner.kind() {
                                "function_declaration"
                                | "class_declaration"
                                | "lexical_declaration"
                                | "variable_declaration"
                                | "import_statement"
                                | "method_definition" => {
                                    push_unit(inner);
                                    found_inner = true;
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    if !found_inner {
                        // Skip re-exports in the MVP
                    }
                }

                _ => {}
            }
        }
    }

    out
}

fn extract_unit_name(node: &Node, code: &str) -> Option<String> {
    match node.kind() {
        // import ... from "module";
        "import_statement" => {
            // Look for the string literal module name
            for i in 0..node.child_count() {
                let c = node.child(i)?;
                let k = c.kind();
                if k == "string" || k == "string_literal" {
                    let raw = c.utf8_text(code.as_bytes()).ok()?.to_string();
                    // strip quotes if present
                    return Some(raw.trim_matches(&['"', '\''][..]).to_string());
                }
            }
            None
        }

        // let/const foo = ..., or variable_declaration forms
        "lexical_declaration" | "variable_declaration" => {
            // find the first identifier under this declaration
            for i in 0..node.child_count() {
                let c = node.child(i)?;
                if c.kind() == "identifier" {
                    return c.utf8_text(code.as_bytes()).ok().map(|s| s.to_string());
                }
                // dig a bit (covers variable_declarator -> identifier)
                if let Some(name) = extract_unit_name(&c, code) {
                    return Some(name);
                }
            }
            None
        }

        // functions/classes/methods — fall back to identifier search
        _ => {
            for i in 0..node.child_count() {
                let c = node.child(i)?;
                if c.kind() == "identifier" || c.kind() == "type_identifier" {
                    return c.utf8_text(code.as_bytes()).ok().map(|s| s.to_string());
                }
                if let Some(name) = extract_unit_name(&c, code) {
                    return Some(name);
                }
            }
            None
        }
    }
}