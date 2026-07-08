//! Built-in example maps, selectable by name.

/// A named, built-in example map.
struct Example {
    /// The name used to select this map on the command line.
    name: &'static str,
    /// The map's ASCII text (see [`crate::grid::Grid::parse`]).
    map: &'static str,
}

/// All built-in example maps. Each is a solvable 5x5 grid.
const EXAMPLES: &[Example] = &[
    Example {
        name: "open",
        map: "S....\n.....\n.....\n.....\n....G\n",
    },
    Example {
        name: "rooms",
        map: "S....\n.###.\n.....\n.###.\n....G\n",
    },
    Example {
        name: "maze",
        map: "S....\n####.\n.....\n.###.\n....G\n",
    },
];

/// The ASCII text of the built-in example named `name`, if it exists.
#[must_use]
pub fn example(name: &str) -> Option<&'static str> {
    EXAMPLES
        .iter()
        .find(|example| example.name == name)
        .map(|example| example.map)
}

/// The names of all built-in example maps.
pub fn names() -> impl Iterator<Item = &'static str> {
    EXAMPLES.iter().map(|example| example.name)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::grid::{Connectivity, Grid};
    use crate::heuristic::Manhattan;
    use crate::search::{Algorithm, search};

    #[test]
    fn every_example_parses_and_is_solvable() {
        for name in names() {
            let text = example(name).expect("listed example exists");
            let grid = Grid::parse(text).expect("example parses");
            assert_eq!((grid.width(), grid.height()), (5, 5), "'{name}' not 5x5");
            let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Four);
            assert!(outcome.path.is_some(), "example '{name}' has no path");
        }
    }

    #[test]
    fn example_returns_the_named_map() {
        assert_eq!(example("open"), Some("S....\n.....\n.....\n.....\n....G\n"));
        assert_eq!(
            example("rooms"),
            Some("S....\n.###.\n.....\n.###.\n....G\n")
        );
        assert_eq!(example("maze"), Some("S....\n####.\n.....\n.###.\n....G\n"));
    }

    #[test]
    fn names_lists_every_example() {
        assert_eq!(names().count(), 3);
        let listed: Vec<&str> = names().collect();
        assert!(listed.contains(&"open"));
        assert!(listed.contains(&"rooms"));
        assert!(listed.contains(&"maze"));
    }

    #[test]
    fn unknown_example_is_none() {
        assert!(example("does-not-exist").is_none());
    }
}
