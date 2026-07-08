# Example Project — A\* Search Simulator

A sample project that demonstrates a **constraints-first, review-gated** way to build
Rust with an AI coding agent. The end product is a command-line A\* search simulator, but
the point of the repository is *how* it was built.

## The process

The project was built in three deliberate phases, each planned before any code was
written.

### 1. Plan for the constraints first

Before a single line of application code, the repo codifies the quality bar so every later change is measured against it:

- **Edition 2024**, pinned stable toolchain ([`rust-toolchain.toml`](rust-toolchain.toml)).
- **Strict lints** in the [`Cargo.toml`](Cargo.toml) `[lints]` table with thresholds in
  [`clippy.toml`](clippy.toml): functions ≤ 20 lines, cognitive complexity ≤ 15, docs on
  **all** items (public *and* private), and a no-panic policy (`unwrap`/`expect`/`panic!`/
  slice-indexing are lints).
- A **pre-commit hook** and a **CI workflow** that both run `cargo fmt --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.

### 2. Plan the `check-rs` quality gate

Next — still before the app — we built a reusable review gate: a project skill at [`.claude/skills/check-rs/`](.claude/skills/check-rs/SKILL.md) backed by four read-only
review sub-agents in [`.claude/agents/`](.claude/agents/):

| Agent | Focus |
|---|---|
| `rust-design-architect` | API/type design, error handling, ownership |
| `rust-dry-reviewer` | duplication and reuse |
| `rust-complexity-docs-reviewer` | complexity and documentation quality |
| `rust-test-coverage-reviewer` | *meaningful* test coverage, not just line counts |

`check-rs` runs `cargo fmt` → `clippy -D warnings` → `cargo test` → measured coverage ([`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov), enforcing **100% of functions**, with `src/main.rs` excluded as the thin entry point), then launches the four agents in parallel and applies every finding.

### 3. Plan and build the app, one gated stage at a time

The simulator was built in six stages — **grid → cost + heuristics → search → map sources → rendering → CLI** — and `check-rs` ran between **every** stage before the stage was committed. The reviews caught real problems that raw coverage missed (for example, a `NaN` random-map density that would have panicked through `rand`). The result is ~95 unit tests and **100% function and line coverage**.

## The application

An A\* search simulator over a 2D grid that compares search strategies and lets you swap
heuristics at runtime.

- **Algorithms:** A\*, greedy best-first, and Dijkstra — one shared search loop that
  differs only in how a node's priority is computed.
- **Heuristics:** Manhattan, Euclidean, Chebyshev, Zero (swap with `--heuristic`).
- **Connectivity:** 4-directional or 8-directional / diagonal (`--connectivity`).
- **Map sources:** built-in examples, an ASCII file (`--map`), or a seeded random
  generator (`--random`).
- **Output:** a step-by-step ANSI animation, or a one-shot `--summary`.

Map glyphs are `S` start, `G` goal, `#` wall, `.` open. In the rendered output, `*` marks the final path, `@` the cell being expanded, `o` the frontier, and `:` visited cells.

## Running it

```sh
# Featured: compare A* vs greedy on the rooms map, with a summary.
cargo run -- --map maps/rooms.txt --compare --heuristic manhattan --summary
```

Output (shown without colour):

```
== A* ==
#####################
#S........#.........#
#*..................#
#*........#.........#
#*****....#.........#
#####*#########.#####
#....*....#.........#
#....*....#.........#
#....*******........#
#.........#********G#
#####################
expanded:     32
max frontier: 28
path cost:    26.00
elapsed:      ...
== greedy ==
#####################
#S........#.........#
#*..................#
#*........#.........#
#*****....#.........#
#####*#########.#####
#....*....#.........#
#....*....#.........#
#....*...***........#
#....*****#********G#
#####################
expanded:     29
max frontier: 26
path cost:    28.00
elapsed:      ...
```

A\* finds the **optimal** path (cost 26), while greedy expands **fewer** nodes (29 vs 32) but returns a **longer** path (cost 28) — the classic greedy-vs-A\* trade-off.

More examples:

```sh
cargo run -- --example open                                   # animated A* solve
cargo run -- --map maps/serpentine.txt --heuristic euclidean  # animate a snake corridor
cargo run -- --random --seed 42 --width 25 --height 15 --summary
cargo run -- --map maps/rooms.txt --connectivity eight --heuristic chebyshev --summary
cargo run -- --map maps/blocked.txt --summary                 # a sealed goal: no path
cargo run -- --help                                           # all options
```

### Sample maps (`maps/`)

| Map | Description |
|---|---|
| `rooms.txt` | Four rooms joined by doorways (best A\* vs greedy contrast) |
| `detour.txt` | A long wall with a single gap |
| `serpentine.txt` | A snake corridor of staggered walls |
| `scatter.txt` | A regular field of pillars |
| `blocked.txt` | A goal sealed on all sides — demonstrates the no-path case |

## Project layout

```
src/lib.rs         library root (all logic, fully tested)
src/grid.rs        grid model, ASCII parsing, MapError
src/cost.rs        ordered Cost newtype (works in a BinaryHeap)
src/heuristic.rs   Heuristic trait + Manhattan/Euclidean/Chebyshev/Zero
src/search.rs      A*/greedy/Dijkstra engine + expansion trace
src/mapgen.rs      seeded random map generation
src/examples.rs    built-in example maps
src/render.rs      step-by-step animation + summary
src/cli.rs         clap command-line interface
src/main.rs        thin entry point
maps/              sample maps for --map
.claude/           the check-rs skill and its review agents
```

## Development setup

### Prerequisites

- Rust (stable). `rustup` reads [`rust-toolchain.toml`](rust-toolchain.toml) and
  auto-installs the pinned toolchain plus the `clippy` and `rustfmt` components.
- For coverage runs: `cargo install cargo-llvm-cov` (the `check-rs` skill installs it on
  first use).

### One-time: install the pre-commit hook

The pre-commit hook is managed by [cargo-husky](https://crates.io/crates/cargo-husky). Install it once by running the test suite:

```sh
cargo test
```

This copies [`.cargo-husky/hooks/pre-commit`](.cargo-husky/hooks/pre-commit) into `.git/hooks/pre-commit`. Re-run `cargo test` after changing the hook script to reinstall it.

### What the hook does

On every `git commit`, the hook:

1. **Auto-formats** staged Rust files with `cargo fmt` and re-stages them, so the commit always contains formatted code.
2. Runs **`cargo clippy --all-targets --all-features -- -D warnings`** as a hard gate. It
   blocks the commit on any lint violation, including:
   - **Function length** — functions over ~20 lines (`too_many_lines`).
   - **Complexity** — overly complex functions (`cognitive_complexity`).
   - **Panics** — `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`,
     and slice indexing that can panic.
   - **Documentation** — undocumented public *and* private items.

Thresholds live in [`clippy.toml`](clippy.toml); the enabled lints live in the `[lints]` table of [`Cargo.toml`](Cargo.toml).

> **Partial staging:** if a staged `.rs` file also has *unstaged* edits, the hook aborts (rather than sweeping the unstaged edits into the commit). Stage the whole file, or run  `cargo fmt` manually, then commit again.

### Tests and the no-panic lints

`unwrap`/`expect`/`panic!` are idiomatic in tests, so allow them at the top of each test module:

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
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)) on every push and pull request,
so bypassed issues surface there anyway.
