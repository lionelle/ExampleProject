//! A\* search simulator library.
//!
//! Provides a 2D grid model and (in later stages) swappable heuristics and
//! search strategies (A\*, greedy best-first) so they can be compared on the
//! command line.

pub mod cli;
pub mod cost;
pub mod examples;
pub mod grid;
pub mod heuristic;
pub mod mapgen;
pub mod render;
pub mod search;
