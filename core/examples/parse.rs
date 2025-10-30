use sink_core::{parse_typescript_to_ast, AstLanguage};

fn main() {
    let code = r#"
        import { log } from './util';
        export function updateUser(name: string) {
            return "Hello, " + name;
        }
        class User { constructor(public id: string) {} }
    "#;

    let ast = parse_typescript_to_ast(code, AstLanguage::TypeScript).unwrap();
    let json = serde_json::to_string_pretty(&ast).unwrap();
    println!("{json}");
}