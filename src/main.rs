//! Command-line entry point for the A\* search simulator.

use example_project::grid::Grid;

/// Parse and print a small built-in demo grid.
fn main() {
    let text = "S....\n.###.\n...#.\n.#.#.\n.#..G\n";
    match Grid::parse(text) {
        Ok(grid) => print!("{grid}"),
        Err(err) => eprintln!("map error: {err}"),
    }
}
