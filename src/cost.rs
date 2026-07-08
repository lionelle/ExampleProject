//! An ordered path/heuristic cost backed by `f64`.

use std::cmp::Ordering;
use std::fmt;
use std::ops::Add;

/// A path or heuristic cost.
///
/// Wraps an `f64` with a *total* order (via [`f64::total_cmp`]) so costs can
/// be stored in ordered containers such as a [`std::collections::BinaryHeap`],
/// which bare `f64` cannot because it is only [`PartialOrd`].
#[derive(Debug, Clone, Copy)]
pub struct Cost(
    /// The wrapped, possibly-fractional cost value.
    f64,
);

impl Cost {
    /// The zero cost.
    pub const ZERO: Self = Self(0.0);

    /// Wrap a raw `f64` cost value.
    ///
    /// The field is private so the ordering wrapper is the only way costs are
    /// compared; a validated constructor can be layered on later without a
    /// breaking change if edge weights ever enter from outside.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// The underlying `f64` value.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

impl PartialEq for Cost {
    /// Equal when the total ordering is [`Ordering::Equal`].
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Cost {}

impl PartialOrd for Cost {
    /// Delegates to the total order defined by [`Ord`].
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cost {
    /// A total order over all `f64` values via [`f64::total_cmp`].
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Add for Cost {
    type Output = Self;

    /// Sum two costs.
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl fmt::Display for Cost {
    /// Format the cost with two decimal places.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn ordering_sorts_ascending() {
        let mut costs = [Cost(3.0), Cost(1.0), Cost(2.0)];
        costs.sort();
        assert_eq!(costs, [Cost(1.0), Cost(2.0), Cost(3.0)]);
        assert!(Cost(1.0) < Cost(2.0));
        assert!(Cost(2.0) > Cost(1.0));
    }

    #[test]
    fn partial_cmp_matches_total_order() {
        assert_eq!(Cost(1.0).partial_cmp(&Cost(2.0)), Some(Ordering::Less));
        assert_eq!(Cost(2.0).partial_cmp(&Cost(2.0)), Some(Ordering::Equal));
    }

    #[test]
    fn equality_holds_and_distinguishes() {
        assert_eq!(Cost(1.5), Cost(1.5));
        assert_ne!(Cost(1.5), Cost(2.5));
    }

    #[test]
    fn add_sums_values() {
        assert_eq!(Cost(1.0) + Cost(2.5), Cost(3.5));
        assert_eq!(Cost::ZERO + Cost(4.0), Cost(4.0));
    }

    #[test]
    fn value_and_zero_expose_inner() {
        assert_eq!(Cost(2.5).value(), 2.5);
        assert_eq!(Cost::ZERO.value(), 0.0);
    }

    #[test]
    fn display_uses_two_decimals() {
        assert_eq!(Cost(1.5).to_string(), "1.50");
        assert_eq!(Cost(2.0).to_string(), "2.00");
    }

    #[test]
    fn cmp_gives_total_order_over_nan_and_signed_zero() {
        // total_cmp ranks NaN above every real value (bare f64 cannot).
        assert!(Cost(f64::NAN) > Cost(f64::INFINITY));
        // NaN is reflexively equal under this total order, unlike raw f64.
        assert_eq!(Cost(f64::NAN).cmp(&Cost(f64::NAN)), Ordering::Equal);
        assert_eq!(Cost(f64::NAN), Cost(f64::NAN));
        // total_cmp distinguishes -0.0 from +0.0 (raw f64 treats them equal).
        assert!(Cost(-0.0) < Cost(0.0));
        assert_ne!(Cost(-0.0), Cost(0.0));
        // Sorting stays deterministic; NaN lands last.
        let mut costs = [Cost(f64::NAN), Cost(2.0), Cost(-1.0)];
        costs.sort();
        assert_eq!(costs, [Cost(-1.0), Cost(2.0), Cost(f64::NAN)]);
    }
}
