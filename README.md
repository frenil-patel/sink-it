Sink‑It (CodeSync) — Semantic Merging for TypeScript

Merge by meaning, not by lines. Sink‑It performs AST‑aware 3‑way merges for TypeScript, auto‑reconciling common “both edited this file” scenarios (e.g., a parameter rename on one branch + a punctuation/text tweak on the other) and hoisting/de‑duping imports — producing clean code with fewer conflicts.

⸻

✨ Why

Traditional Git merges are text‑based. They flag conflicts even when two edits are semantically compatible. That wastes time and breaks developer flow. Sink‑It parses code into an Abstract Syntax Tree (AST) and composes changes at the code‑structure level — safely auto‑merging the easy/medium cases and only raising conflicts when unsure.

⸻

✅ What it does (MVP)
	•	AST‑based 3‑way merge for TypeScript (.ts/.tsx)
	•	Precise splicing of changed top‑level units (functions/classes/vars)
	•	Rename‑aware merges for simple function parameter renames
	•	Import union: de‑dupes and hoists imports to the top of the file
	•	Conservative policy: when ambiguous, emits a clear conflict (no risky guesses)
	•	Git‑aware CLI: reads refs from your repo and writes merged results to ./.codesync/

Output is written to .codesync/ so your working tree remains untouched.

⸻

🚀 Quick demo (copy‑paste)

# 1) Create a tiny TS repo with two branches
mkdir -p demo-ts && cd demo-ts && git init && mkdir -p src
cat > src/user.ts <<'EOF'
export function updateUser(name: string) {
  return "Hello " + name;
}
EOF
git add . && git commit -m "base"

git checkout -b feature/a
cat > src/user.ts <<'EOF'
import { log } from "./util";
export function updateUser(userName: string) {
  return "Hello " + userName;
}
EOF
echo 'export const log = (..._args:any[])=>{}' > src/util.ts
git add . && git commit -m "A: rename + import"

git checkout -b feature/b main
cat > src/user.ts <<'EOF'
export function updateUser(name: string) {
  return "Hello, " + name + "!";
}
EOF
git add . && git commit -m "B: punctuation"

# 2) From the Sink‑It Rust project folder (the one with Cargo.toml for core)
cd ../core
cargo run --bin sinkit -- ../demo-ts feature/a feature/b

# 3) Inspect result
sed -n '1,160p' .codesync/src__user.ts

Expected output:

import { log } from "./util";

export function updateUser(userName: string) {
  return "Hello, " + userName + "!";
}


⸻

🧩 How it works (architecture)
	1.	Parse (tree‑sitter) → Build a minimal top‑level AST for TS/TSX.
	2.	Diff Base→A and Base→B at the unit level (function/class/import/var).
	3.	Compose the edit scripts:
	•	Splice accepted updates by byte range from the Base AST
	•	Reconcile param renames (simple heuristic) to combine otherwise conflicting edits
	•	Union imports and place them at the top (de‑duped)
	4.	Conservative fallback: if both branches change the same unit incompatibly → emit a conflict reason.

Key crates: tree-sitter, tree-sitter-typescript, serde, anyhow.

⸻

🛠️ Install / Build

Requirements: Rust (via rustup) and git.

# in the repo root
cargo build

If cargo isn’t found, run: . "$HOME/.cargo/env" then cargo --version.

⸻

🧪 CLI usage

sinkit <repo_path> <A_ref> <B_ref>

Examples:

# run via cargo
cargo run --bin sinkit -- ~/code/my-ts-repo feature/a feature/b

# after building
./target/debug/sinkit ~/code/my-ts-repo feature/a feature/b

Results are written to ./.codesync/ (relative to the current working directory). Files keep path semantics using __ in place of / (e.g., src__user.ts).

⸻

⚠️ Current scope & limitations
	•	Top‑level units only (functions/classes/imports/var decls)
	•	Rename‑aware merge only for first function parameter; other signature/inside‑body edits may still conflict
	•	Import union is line‑level (does not yet coalesce {a} + {b} into {a, b})
	•	No cross‑file refactor detection (no TS symbol graph yet)
	•	TSX is parsed; name extraction for default exports/HOCs may be conservative

Design is intentionally conservative: when unsure, emit a conflict instead of risking broken code.

⸻

🗺️ Roadmap
	•	Import specifier coalescing (import {a} + import {b} → import {a, b})
	•	Statement‑level tree diff (GumTree‑style) to reconcile edits inside function bodies
	•	TypeScript compiler integration (ts‑morph) for robust rename detection across files
	•	Method‑level splicing inside classes
	•	VS Code extension & GitHub Action wrappers
	•	Multi‑language support (add parsers: Go/Java/Python)

⸻

🤝 Contributing

PRs welcome! Please:
	•	Keep changes small and well‑commented
	•	Add a demo case under core/examples/ if relevant
	•	Prefer conservative merges over risky heuristics

⸻

📄 License

MIT — see LICENSE.

MIT License

Copyright (c) 2025 Sink‑It contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.


⸻

🙋 FAQ

Does this replace Git? No. It’s a smarter merge layer you can run before/after a normal merge.

Will it solve all conflicts? No. It automates safe, common cases and surfaces the rest.

Does it change my working tree? No — results go to .codesync/.
