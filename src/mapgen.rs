//! Seeded procedural generation of random grid maps.

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::grid::{GOAL, Grid, MapError, OPEN, START, WALL};

/// Parameters for generating a random map.
#[derive(Debug, Clone, Copy)]
pub struct MapSpec {
    /// Number of columns.
    pub width: usize,
    /// Number of rows.
    pub height: usize,
    /// Probability each cell other than the start and goal becomes a wall.
    /// Clamped to `0.0..=1.0`; a `NaN` is treated as `0.0`.
    pub wall_density: f64,
    /// Seed for reproducible output.
    pub seed: u64,
}

impl MapSpec {
    /// Generate a random [`Grid`] from this spec.
    ///
    /// The start is the top-left cell and the goal the bottom-right; both are
    /// forced open. Every other cell — borders included — becomes a wall with
    /// probability `wall_density`. The same `seed`, dimensions, and
    /// `wall_density` always yield the same map. Solvability is not guaranteed —
    /// a blocked map simply yields no path when searched.
    ///
    /// # Errors
    /// Returns a [`MapError`] if the dimensions cannot hold a distinct start
    /// and goal (e.g. a zero or single-cell grid).
    pub fn generate(self) -> Result<Grid, MapError> {
        Grid::parse(&self.render_text())
    }

    /// The wall probability, sanitised into `0.0..=1.0` (a `NaN` becomes `0.0`).
    ///
    /// `f64::clamp` passes `NaN` through unchanged, and `rand`'s `random_bool`
    /// panics on a `NaN` probability, so `NaN` must be handled explicitly.
    fn density(self) -> f64 {
        if self.wall_density.is_nan() {
            0.0
        } else {
            self.wall_density.clamp(0.0, 1.0)
        }
    }

    /// Render the spec to map text (start/goal fixed, other cells seeded).
    fn render_text(self) -> String {
        let mut rng = StdRng::seed_from_u64(self.seed);
        let density = self.density();
        let mut text = String::with_capacity((self.width + 1) * self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                text.push(self.cell_char(&mut rng, density, x, y));
            }
            text.push('\n');
        }
        text
    }

    /// The glyph for cell `(x, y)`: start, goal, or a seeded wall/open cell.
    fn cell_char(self, rng: &mut StdRng, density: f64, x: usize, y: usize) -> char {
        let goal = (self.width.saturating_sub(1), self.height.saturating_sub(1));
        if (x, y) == (0, 0) {
            START
        } else if (x, y) == goal {
            GOAL
        } else if rng.random_bool(density) {
            WALL
        } else {
            OPEN
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::grid::Pos;

    fn spec(wall_density: f64, seed: u64) -> MapSpec {
        MapSpec {
            width: 8,
            height: 6,
            wall_density,
            seed,
        }
    }

    #[test]
    fn same_seed_is_reproducible() {
        let a = spec(0.3, 42).generate().unwrap();
        let b = spec(0.3, 42).generate().unwrap();
        assert_eq!(a.to_string(), b.to_string());
    }

    #[test]
    fn different_seed_changes_the_map() {
        let a = spec(0.3, 1).generate().unwrap();
        let b = spec(0.3, 2).generate().unwrap();
        assert_ne!(a.to_string(), b.to_string());
    }

    #[test]
    fn dimensions_and_endpoints_match_spec() {
        let grid = spec(0.3, 7).generate().unwrap();
        assert_eq!((grid.width(), grid.height()), (8, 6));
        assert_eq!(grid.start(), Pos::new(0, 0));
        assert_eq!(grid.goal(), Pos::new(7, 5));
        assert!(grid.is_open(grid.start()) && grid.is_open(grid.goal()));
    }

    #[test]
    fn zero_density_has_no_interior_walls() {
        let grid = spec(0.0, 9).generate().unwrap();
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                assert!(grid.is_open(Pos::new(x, y)));
            }
        }
    }

    #[test]
    fn full_density_walls_every_non_endpoint_cell() {
        let grid = spec(1.0, 9).generate().unwrap();
        assert!(!grid.is_open(Pos::new(1, 1))); // interior
        assert!(!grid.is_open(Pos::new(1, 0))); // top border
        assert!(!grid.is_open(Pos::new(0, 1))); // left border
        assert!(grid.is_open(grid.start()));
        assert!(grid.is_open(grid.goal()));
    }

    #[test]
    fn wall_density_above_one_clamps_to_full() {
        let clamped = spec(2.0, 9).generate().unwrap();
        let full = spec(1.0, 9).generate().unwrap();
        assert_eq!(clamped.to_string(), full.to_string());
    }

    #[test]
    fn wall_density_below_zero_clamps_to_empty() {
        let clamped = spec(-0.5, 9).generate().unwrap();
        let empty = spec(0.0, 9).generate().unwrap();
        assert_eq!(clamped.to_string(), empty.to_string());
    }

    #[test]
    fn nan_density_is_treated_as_zero_without_panicking() {
        let nan = spec(f64::NAN, 9).generate().unwrap();
        let empty = spec(0.0, 9).generate().unwrap();
        assert_eq!(nan.to_string(), empty.to_string());
    }

    #[test]
    fn too_small_dimensions_error() {
        let tiny = MapSpec {
            width: 1,
            height: 1,
            wall_density: 0.0,
            seed: 0,
        };
        assert!(tiny.generate().is_err());
    }
}
