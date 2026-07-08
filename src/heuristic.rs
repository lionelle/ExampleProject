//! Swappable heuristics estimating the remaining cost between two positions.

use crate::cost::Cost;
use crate::grid::Pos;

/// Estimates the cost of travelling from one position to another.
///
/// A heuristic that never overestimates the true remaining cost is
/// *admissible*, which keeps A\* optimal. Admissibility depends on the grid's
/// [`Connectivity`](crate::grid::Connectivity) and step costs — see each
/// implementor's note (e.g. `Manhattan` is admissible only on a 4-connected
/// grid). Implementors are cheap, stateless value types so they can be
/// selected at runtime as `&dyn Heuristic`.
pub trait Heuristic {
    /// Estimate the cost from `from` to `to`.
    fn estimate(&self, from: Pos, to: Pos) -> Cost;
}

/// The absolute `(dx, dy)` between two positions, as floating point.
fn deltas(from: Pos, to: Pos) -> (f64, f64) {
    // usize -> f64 is exact for any coordinate a grid could realistically hold.
    (from.x.abs_diff(to.x) as f64, from.y.abs_diff(to.y) as f64)
}

/// Manhattan distance `|dx| + |dy|`; admissible on a 4-connected unit grid.
#[derive(Debug, Clone, Copy, Default)]
pub struct Manhattan;

impl Heuristic for Manhattan {
    fn estimate(&self, from: Pos, to: Pos) -> Cost {
        let (dx, dy) = deltas(from, to);
        Cost::new(dx + dy)
    }
}

/// Straight-line (L2) distance `sqrt(dx^2 + dy^2)`; admissible on any grid.
#[derive(Debug, Clone, Copy, Default)]
pub struct Euclidean;

impl Heuristic for Euclidean {
    fn estimate(&self, from: Pos, to: Pos) -> Cost {
        let (dx, dy) = deltas(from, to);
        Cost::new(dx.hypot(dy))
    }
}

/// Chebyshev (L∞) distance `max(|dx|, |dy|)`; admissible on an 8-connected
/// unit grid.
#[derive(Debug, Clone, Copy, Default)]
pub struct Chebyshev;

impl Heuristic for Chebyshev {
    fn estimate(&self, from: Pos, to: Pos) -> Cost {
        let (dx, dy) = deltas(from, to);
        Cost::new(dx.max(dy))
    }
}

/// The zero heuristic; turns A\* into Dijkstra's algorithm.
#[derive(Debug, Clone, Copy, Default)]
pub struct Zero;

impl Heuristic for Zero {
    fn estimate(&self, _from: Pos, _to: Pos) -> Cost {
        Cost::ZERO
    }
}

/// A runtime-selectable heuristic, e.g. chosen from a command-line flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeuristicKind {
    /// Selects [`Manhattan`].
    Manhattan,
    /// Selects [`Euclidean`].
    Euclidean,
    /// Selects [`Chebyshev`].
    Chebyshev,
    /// Selects [`Zero`].
    Zero,
}

impl HeuristicKind {
    /// Build the boxed [`Heuristic`] this kind names.
    #[must_use]
    pub fn build(self) -> Box<dyn Heuristic> {
        match self {
            Self::Manhattan => Box::new(Manhattan),
            Self::Euclidean => Box::new(Euclidean),
            Self::Chebyshev => Box::new(Chebyshev),
            Self::Zero => Box::new(Zero),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// Origin and a 3-4-5 offset position reused across the value tests.
    const ORIGIN: Pos = Pos::new(0, 0);
    const FAR: Pos = Pos::new(3, 4);

    #[test]
    fn manhattan_sums_absolute_deltas() {
        assert_eq!(Manhattan.estimate(ORIGIN, FAR), Cost::new(7.0));
    }

    #[test]
    fn euclidean_is_straight_line_distance() {
        assert_eq!(Euclidean.estimate(ORIGIN, FAR), Cost::new(5.0));
    }

    #[test]
    fn chebyshev_takes_the_larger_delta() {
        assert_eq!(Chebyshev.estimate(ORIGIN, FAR), Cost::new(4.0));
    }

    #[test]
    fn zero_is_always_zero() {
        assert_eq!(Zero.estimate(ORIGIN, FAR), Cost::ZERO);
    }

    #[test]
    fn every_heuristic_is_zero_at_the_goal() {
        let here = Pos::new(5, 2);
        for kind in [
            HeuristicKind::Manhattan,
            HeuristicKind::Euclidean,
            HeuristicKind::Chebyshev,
            HeuristicKind::Zero,
        ] {
            assert_eq!(kind.build().estimate(here, here), Cost::ZERO);
        }
    }

    #[test]
    fn estimates_are_symmetric() {
        let h = Euclidean;
        assert_eq!(h.estimate(ORIGIN, FAR), h.estimate(FAR, ORIGIN));
    }

    #[test]
    fn estimate_orders_zero_le_chebyshev_le_euclidean_le_manhattan() {
        let pairs = [
            (Pos::new(0, 0), Pos::new(3, 4)),
            (Pos::new(7, 1), Pos::new(2, 6)),
            (Pos::new(5, 5), Pos::new(5, 9)),
            (Pos::new(5, 5), Pos::new(9, 5)),
            (Pos::new(10, 10), Pos::new(10, 10)),
        ];
        for (from, to) in pairs {
            let z = Zero.estimate(from, to).value();
            let c = Chebyshev.estimate(from, to).value();
            let e = Euclidean.estimate(from, to).value();
            let m = Manhattan.estimate(from, to).value();
            assert!(z <= c && c <= e && e <= m, "broke for {from:?}->{to:?}");
        }
    }

    #[test]
    fn deltas_use_absolute_difference_in_both_directions() {
        // from > to on both axes; the same 3-4-5 triangle as ORIGIN->FAR.
        let a = Pos::new(7, 6);
        let b = Pos::new(4, 2);
        assert_eq!(Manhattan.estimate(a, b), Cost::new(7.0));
        assert_eq!(Euclidean.estimate(a, b), Cost::new(5.0));
        assert_eq!(Chebyshev.estimate(a, b), Cost::new(4.0));
        assert_eq!(Manhattan.estimate(b, a), Cost::new(7.0));
        assert_eq!(Chebyshev.estimate(b, a), Cost::new(4.0));
    }

    #[test]
    fn build_dispatches_to_each_variant() {
        assert_eq!(
            HeuristicKind::Manhattan.build().estimate(ORIGIN, FAR),
            Cost::new(7.0)
        );
        assert_eq!(
            HeuristicKind::Euclidean.build().estimate(ORIGIN, FAR),
            Cost::new(5.0)
        );
        assert_eq!(
            HeuristicKind::Chebyshev.build().estimate(ORIGIN, FAR),
            Cost::new(4.0)
        );
        assert_eq!(
            HeuristicKind::Zero.build().estimate(ORIGIN, FAR),
            Cost::ZERO
        );
    }
}
