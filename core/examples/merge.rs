use sink_core::{three_way_merge_top_level, AstLanguage};

fn main() {
    let base = r#"
export function updateUser(name: string) {
  return "Hello " + name;
}
"#;

    let a = r#"
import { log } from "./util";

export function updateUser(userName: string) {
  return "Hello " + userName;
}
"#;

    let b = r#"
export function updateUser(name: string) {
  return "Hello, " + name + "!";
}
"#;

    let res = three_way_merge_top_level(base, a, b, AstLanguage::TypeScript).unwrap();
    println!("--- MERGED CODE ---\n{}", res.merged_code);
    if res.conflicts.is_empty() {
        println!("(no conflicts)");
    } else {
        println!("--- CONFLICTS ---");
        for c in res.conflicts {
            println!("- {c}");
        }
    }
}