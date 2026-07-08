# Example Project

A new Rust application.

## Development setup

### Prerequisites

- Rust (stable). `rustup` reads [`rust-toolchain.toml`](rust-toolchain.toml) and
  auto-installs the pinned toolchain plus the `clippy` and `rustfmt` components.

### One-time: install the pre-commit hook

The pre-commit hook is managed by
[cargo-husky](https://crates.io/crates/cargo-husky). Install it once by running
the test suite:

```sh
cargo test
```

This copies [`.cargo-husky/hooks/pre-commit`](.cargo-husky/hooks/pre-commit)
into `.git/hooks/pre-commit`. Re-run `cargo test` after changing the hook script
to reinstall it.

### What the hook does

On every `git commit`, the hook:

1. **Auto-formats** staged Rust files with `cargo fmt` and re-stages them, so
   the commit always contains formatted code.
2. Runs **`cargo clippy --all-targets --all-features -- -D warnings`** as a hard
   gate. It blocks the commit on any lint violation, including:
   - **Function length** — functions over ~20 lines (`too_many_lines`).
   - **Complexity** — overly complex functions (`cognitive_complexity`).
   - **Panics** — `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`,
     `unreachable!`, and slice indexing that can panic.
   - **Documentation** — undocumented public *and* private items.

Thresholds live in [`clippy.toml`](clippy.toml); the enabled lints live in the
`[lints]` table of [`Cargo.toml`](Cargo.toml).

> **Partial staging:** if a staged `.rs` file also has *unstaged* edits, the
> hook aborts (rather than sweeping the unstaged edits into the commit). Stage
> the whole file, or run `cargo fmt` manually, then commit again.

### Tests and the no-panic lints

`unwrap`/`expect`/`panic!` are idiomatic in tests, so allow them at the top of
each test module:

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    // ...
}
```

### Bypassing the hook

In a genuine emergency you can skip the hook with:

```sh
git commit --no-verify
```

This is discouraged — the same checks run in CI
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) on every push and pull
request, so bypassed issues surface there anyway.
