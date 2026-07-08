//! A\* search simulator library.
//!
//! Provides a 2D grid model and (in later stages) swappable heuristics and
//! search strategies (A\*, greedy best-first) so they can be compared on the
//! command line.

pub mod cost;
pub mod grid;
pub mod heuristic;
