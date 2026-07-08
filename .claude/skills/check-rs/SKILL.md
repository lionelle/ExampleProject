---
name: check-rs
description: Rust Code Quality Pass — fmt, clippy, measured coverage (cargo-llvm-cov), then design/DRY/docs/test review agents, then fixes all findings
user-invocable: true
argument-hint: "[all | <path>...]"
---

# /check-rs — Rust Code Quality Pass

Perform a comprehensive quality pass on this project's Rust code. **Fix every issue found — do not just
report them.**

This skill is tuned to *this repo's* codified standards (do not substitute generic defaults):

- **Gate commands** mirror CI (`.github/workflows/ci.yml`) and the pre-commit hook
  (`.cargo-husky/hooks/pre-commit`).
- **Thresholds** come from `clippy.toml`: **functions ≤ 20 lines**, **cognitive complexity ≤ 15**.
- **Docs required on ALL items — public and private** (`missing_docs`, `missing_docs_in_private_items`),
  with `# Errors` / `# Panics` sections where relevant.
- **No-panic policy**: `unwrap`/`expect`/`panic!`/`todo!`/`unimplemented!`/`unreachable!`/indexing are
  clippy-lints; Phase 2 catches them. In tests, allow them with the header shown in Phase 4.
- **Coverage targets** (tune here): `--fail-under-functions 100`, `--fail-under-lines 90`.

## Phase 1: Determine scope

Interpret the argument:

- **No argument** — review git-changed and untracked `.rs` files: run `git diff HEAD --stat` and
  `git status --short`. Exclude `Cargo.lock` and `target/`. If there are no changes, ask the user which
  files to review (or suggest `/check-rs all`).
- **`all`** — review every `.rs` file under `src/` (and `tests/` if present). Use Glob.
- **Explicit path(s)** — review exactly those files.

State the resolved file list before proceeding.

## Phase 2: Run the tooling (fix before reviewing)

Run these and fix every problem before launching agents. Do not skip ahead if a tool reports issues.

**Formatting** (apply, don't just check):
```
cargo fmt --all
```

**Linting** (warnings are errors — matches CI and the hook):
```
cargo clippy --all-targets --all-features -- -D warnings
```
Fix every warning in the reviewed files and any file you touch while fixing. Do **not** add `#[allow(...)]`
to silence a warning unless the suppression carries a doc comment explaining why it is necessary. This
step mechanically enforces the no-panic lints, so you do not need the agents to re-detect raw
`unwrap`/`expect`/`panic!`/indexing.

Confirm a clean build and passing tests:
```
cargo test
```

## Phase 2b: Coverage baseline

Ensure `cargo-llvm-cov` is available; install it once if missing (say so in the summary):
```
cargo llvm-cov --version || { rustup component add llvm-tools-preview && cargo install cargo-llvm-cov; }
```
Capture the current coverage to hand to the test agent:
```
cargo llvm-cov --all-features --ignore-filename-regex 'src/main\.rs' --summary-only
```
Also capture a per-function/line breakdown (e.g. `cargo llvm-cov --all-features --ignore-filename-regex
'src/main\.rs' --text` or `--json`) and save the report text — you will pass it to the test-coverage agent
in Phase 3 and reuse the starting % in the summary.

The `--ignore-filename-regex 'src/main\.rs'` excludes the binary entrypoint (`fn main`), which
`cargo test` never enters, from the function-coverage gate. Keep all real logic in `src/lib.rs` (or other
modules) so it stays measured; `src/main.rs` must be a thin one-function shell.

## Phase 3: Launch the four review agents in parallel

Use the Agent tool to launch **all four in a single message** (parallel). Give each the resolved file list
and the full contents of those files. Additionally give the test-coverage agent the Phase 2b coverage
report.

- `subagent_type: rust-design-architect` — API/type design, error-handling design, ownership ergonomics,
  module boundaries, visibility, naming.
- `subagent_type: rust-dry-reviewer` — duplication, reuse of existing helpers/std/crates, magic literals
  → constants, misplaced items.
- `subagent_type: rust-complexity-docs-reviewer` — functions > 20 lines, cognitive complexity > 15, every
  undocumented item (public **and** private), `# Errors`/`# Panics` sections, comment hygiene.
- `subagent_type: rust-test-coverage-reviewer` — enumerate functions, cross-reference the llvm-cov report,
  write concrete missing tests toward 100% function coverage.

## Phase 4: Fix all findings

Wait for all four agents, then work through every finding:

- Apply fixes directly with Edit/Write.
- Add tests in an inline module using this repo's required allow-header:
  ```rust
  #[cfg(test)]
  mod tests {
      #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
      use super::*;
      // ...
  }
  ```
- If a finding is a clear false positive, note it and skip it.
- Do not add `#[allow(...)]` without a justifying doc comment.
- Do not add tests that assert nothing meaningful.

## Phase 5: Final verification

Run in sequence — all must pass cleanly:
```
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo llvm-cov --all-features --ignore-filename-regex 'src/main\.rs' --fail-under-functions 100 --fail-under-lines 90
```
If any fail, fix and re-run before declaring done. (Adjust the two `--fail-under-*` values only with the
user's agreement — they encode the project's coverage bar.)

## Summary

Report what was fixed in each category; if a category had no issues, say so explicitly:

- **Tooling** — fmt/clippy changes made.
- **Design** — architecture/API/error-handling/ownership changes.
- **DRY / Organisation** — duplications removed, constants added, code moved.
- **Complexity / Docs** — functions split, docs added, comments trimmed.
- **Tests / Coverage** — tests added (list them) and coverage **before → after** (%).

Also note whether `cargo-llvm-cov` had to be installed.
