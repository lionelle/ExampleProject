//! The search engine: A\*, greedy best-first, and Dijkstra over a [`Grid`].
//!
//! All three strategies share one loop; they differ only in how [`Algorithm`]
//! turns the path cost `g` and heuristic estimate `h` into a priority. A search
//! records an expansion trace so a UI can replay how the frontier grew.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::cost::Cost;
use crate::grid::{Connectivity, Grid, Pos};
use crate::heuristic::Heuristic;

/// The search strategy, i.e. how a node's priority is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// A\*: order by `f = g + h`.
    AStar,
    /// Greedy best-first: order by `h` alone, ignoring path cost.
    Greedy,
    /// Dijkstra: order by `g` alone, ignoring the heuristic.
    Dijkstra,
}

impl Algorithm {
    /// The priority (lowest is expanded first) for path cost `g` and estimate
    /// `h`.
    fn priority(self, g: Cost, h: Cost) -> Cost {
        match self {
            Self::AStar => g + h,
            Self::Greedy => h,
            Self::Dijkstra => g,
        }
    }
}

/// One recorded expansion, for animation/replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The position expanded on this step.
    pub expanded: Pos,
    /// Positions (re)enqueued while expanding, in discovery order. A position
    /// reappears here if this expansion found a strictly cheaper path to it.
    pub enqueued: Vec<Pos>,
}

/// Aggregate statistics for a completed search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    /// The number of nodes expanded.
    pub expanded: usize,
    /// The peak number of open-set entries (counts stale duplicates).
    pub max_frontier: usize,
    /// The cost of the returned path, or `None` if the goal is unreachable.
    pub path_cost: Option<Cost>,
}

/// The result of a search: the path if any, the expansion trace, and stats.
#[derive(Debug, Clone)]
pub struct SearchOutcome {
    /// The path from start to goal, or `None` if the goal is unreachable.
    pub path: Option<Vec<Pos>>,
    /// Per-expansion trace for animation; the canonical record of the search.
    pub steps: Vec<Step>,
    /// Aggregate statistics.
    pub stats: Stats,
}

impl SearchOutcome {
    /// The positions expanded, in expansion order (derived from [`Self::steps`]).
    pub fn expanded_order(&self) -> impl Iterator<Item = Pos> + '_ {
        self.steps.iter().map(|step| step.expanded)
    }
}

/// Search `grid` from its start to its goal with `algo` and `heuristic`.
///
/// The search is infallible: an unreachable goal yields
/// [`SearchOutcome::path`] of `None` (with the trace still populated).
///
/// A\* and Dijkstra return an optimal path only when the heuristic is
/// *consistent* for the chosen [`Connectivity`]. The provided heuristics are
/// consistent on 4-connected grids; pairing [`Manhattan`](crate::heuristic::Manhattan)
/// with [`Connectivity::Eight`] is inadmissible, so the no-reopen closed set
/// may then return a sub-optimal path.
///
/// # Examples
/// ```
/// use example_project::grid::{Connectivity, Grid};
/// use example_project::heuristic::Manhattan;
/// use example_project::search::{search, Algorithm};
/// let grid = Grid::parse("S.\n.G\n").unwrap();
/// let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Four);
/// assert!(outcome.path.is_some());
/// ```
#[must_use]
pub fn search(
    grid: &Grid,
    heuristic: &dyn Heuristic,
    algo: Algorithm,
    conn: Connectivity,
) -> SearchOutcome {
    let mut explorer = Explorer::new(grid, heuristic, algo, conn);
    let path = explorer.run();
    explorer.into_outcome(path)
}

/// A queued node awaiting expansion.
#[derive(Debug)]
struct Frontier {
    /// The value the algorithm orders by (lowest is expanded first).
    priority: Cost,
    /// Path cost from the start to `pos`.
    g: Cost,
    /// Insertion sequence, a FIFO tie-breaker.
    seq: u64,
    /// The queued position.
    pos: Pos,
}

impl Ord for Frontier {
    /// Ordered so a max-[`BinaryHeap`] pops the lowest priority first, breaking
    /// ties toward higher `g` (fewer expansions) then earlier insertion.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| self.g.cmp(&other.g))
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Frontier {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Frontier {}

/// Runs a single search, recording a trace for later replay.
struct Explorer<'a> {
    /// The grid being searched.
    grid: &'a Grid,
    /// The heuristic estimating remaining cost.
    heuristic: &'a dyn Heuristic,
    /// The strategy combining `g` and `h` into a priority.
    algo: Algorithm,
    /// Movement connectivity.
    conn: Connectivity,
    /// The goal position.
    goal: Pos,
    /// The open set (priority queue).
    open: BinaryHeap<Frontier>,
    /// Best known path cost to each reached position.
    g: HashMap<Pos, Cost>,
    /// Predecessor of each reached position, for reconstruction.
    came_from: HashMap<Pos, Pos>,
    /// Positions already expanded (the closed set).
    closed: HashSet<Pos>,
    /// Per-expansion trace.
    steps: Vec<Step>,
    /// Peak number of open-set entries seen.
    max_open: usize,
    /// Monotonic counter assigning each push a FIFO sequence number.
    seq: u64,
}

impl<'a> Explorer<'a> {
    /// Create an explorer seeded with the grid's start at cost zero.
    fn new(
        grid: &'a Grid,
        heuristic: &'a dyn Heuristic,
        algo: Algorithm,
        conn: Connectivity,
    ) -> Self {
        let start = grid.start();
        let mut explorer = Self {
            grid,
            heuristic,
            algo,
            conn,
            goal: grid.goal(),
            open: BinaryHeap::new(),
            g: HashMap::new(),
            came_from: HashMap::new(),
            closed: HashSet::new(),
            steps: Vec::new(),
            max_open: 0,
            seq: 0,
        };
        // The start's cost is supplied by `cost_to`'s zero fallback, so it is
        // intentionally not inserted into `g` here.
        explorer.push(start, Cost::ZERO);
        explorer
    }

    /// Expand nodes until the goal is reached or the open set empties.
    fn run(&mut self) -> Option<Vec<Pos>> {
        while let Some(top) = self.open.pop() {
            if !self.closed.insert(top.pos) {
                // Lazy deletion: with no decrease-key, a cheaper re-push left
                // this now-stale entry behind, so skip it.
                continue;
            }
            let reached_goal = top.pos == self.goal;
            let enqueued = if reached_goal {
                Vec::new()
            } else {
                self.expand(top.pos)
            };
            self.record_step(top.pos, enqueued);
            if reached_goal {
                return Some(self.reconstruct());
            }
        }
        None
    }

    /// Record one expansion and update the peak open-set size.
    fn record_step(&mut self, expanded: Pos, enqueued: Vec<Pos>) {
        self.max_open = self.max_open.max(self.open.len());
        self.steps.push(Step { expanded, enqueued });
    }

    /// Relax every neighbour of `pos`; return the newly enqueued positions.
    fn expand(&mut self, pos: Pos) -> Vec<Pos> {
        let mut enqueued = Vec::new();
        for next in self.grid.neighbors(pos, self.conn) {
            if self.relax(pos, next) {
                enqueued.push(next);
            }
        }
        enqueued
    }

    /// Try to improve the path to `next` via `from`; return whether it was
    /// (re)enqueued.
    fn relax(&mut self, from: Pos, next: Pos) -> bool {
        if self.closed.contains(&next) {
            return false;
        }
        let tentative = self.cost_to(from) + self.grid.step_cost(from, next);
        if self.g.get(&next).is_some_and(|&best| tentative >= best) {
            return false;
        }
        self.g.insert(next, tentative);
        self.came_from.insert(next, from);
        self.push(next, tentative);
        true
    }

    /// The best known path cost to `pos`.
    ///
    /// Falls back to zero, which by design only the start position hits: the
    /// start is never inserted into `g`, and every other `pos` passed here has
    /// already been relaxed (so it is present).
    fn cost_to(&self, pos: Pos) -> Cost {
        self.g.get(&pos).copied().unwrap_or(Cost::ZERO)
    }

    /// Enqueue `pos` with path cost `g`, computing its priority.
    fn push(&mut self, pos: Pos, g: Cost) {
        let h = self.heuristic.estimate(pos, self.goal);
        let priority = self.algo.priority(g, h);
        self.open.push(Frontier {
            priority,
            g,
            seq: self.seq,
            pos,
        });
        self.seq += 1;
    }

    /// Rebuild the path from start to goal by walking `came_from`.
    fn reconstruct(&self) -> Vec<Pos> {
        let mut path = vec![self.goal];
        let mut cur = self.goal;
        while let Some(&prev) = self.came_from.get(&cur) {
            path.push(prev);
            cur = prev;
        }
        path.reverse();
        path
    }

    /// Consume the explorer into a [`SearchOutcome`].
    fn into_outcome(self, path: Option<Vec<Pos>>) -> SearchOutcome {
        let path_cost = path.as_ref().map(|_| self.cost_to(self.goal));
        let stats = Stats {
            expanded: self.steps.len(),
            max_frontier: self.max_open,
            path_cost,
        };
        SearchOutcome {
            path,
            steps: self.steps,
            stats,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::heuristic::{Euclidean, Manhattan, Zero};

    fn solve(map: &str, algo: Algorithm) -> SearchOutcome {
        let grid = Grid::parse(map).expect("map should parse");
        search(&grid, &Manhattan, algo, Connectivity::Four)
    }

    /// Build a frontier node with a throwaway position for ordering tests.
    fn frontier(priority: f64, g: f64, seq: u64) -> Frontier {
        Frontier {
            priority: Cost::new(priority),
            g: Cost::new(g),
            seq,
            pos: Pos::new(0, 0),
        }
    }

    /// A 3x3 grid whose only detour is around two walls; optimal cost is 4.
    const DETOUR: &str = "S.#\n...\n#.G\n";

    #[test]
    fn astar_finds_shortest_path_on_open_grid() {
        let outcome = solve("S..\n...\n..G\n", Algorithm::AStar);
        let path = outcome.path.expect("path exists");
        assert_eq!(path.first(), Some(&Pos::new(0, 0)));
        assert_eq!(path.last(), Some(&Pos::new(2, 2)));
        assert_eq!(outcome.stats.path_cost, Some(Cost::new(4.0)));
    }

    #[test]
    fn astar_and_dijkstra_find_equal_optimal_cost() {
        let a = solve(DETOUR, Algorithm::AStar);
        let d = solve(DETOUR, Algorithm::Dijkstra);
        assert_eq!(a.stats.path_cost, Some(Cost::new(4.0)));
        assert_eq!(a.stats.path_cost, d.stats.path_cost);
    }

    #[test]
    fn greedy_path_is_not_cheaper_than_astar() {
        let a = solve(DETOUR, Algorithm::AStar);
        let greedy = solve(DETOUR, Algorithm::Greedy);
        let (ac, gc) = (a.stats.path_cost.unwrap(), greedy.stats.path_cost.unwrap());
        assert!(gc >= ac, "greedy {gc} cheaper than A* {ac}");
    }

    #[test]
    fn greedy_expands_no_more_than_astar_on_open_grid() {
        let open = "S....\n.....\n.....\n.....\n....G\n";
        let a = solve(open, Algorithm::AStar);
        let greedy = solve(open, Algorithm::Greedy);
        assert!(greedy.stats.expanded <= a.stats.expanded);
    }

    #[test]
    fn unreachable_goal_yields_no_path() {
        let outcome = solve("S#G\n", Algorithm::AStar);
        assert!(outcome.path.is_none());
        assert_eq!(outcome.stats.path_cost, None);
        assert_eq!(outcome.expanded_order().next(), Some(Pos::new(0, 0)));
    }

    #[test]
    fn trace_starts_at_start_and_matches_stats() {
        let outcome = solve(DETOUR, Algorithm::AStar);
        assert_eq!(outcome.expanded_order().next(), Some(Pos::new(0, 0)));
        assert_eq!(outcome.expanded_order().count(), outcome.stats.expanded);
        assert!(outcome.stats.max_frontier > 0);
    }

    #[test]
    fn path_is_contiguous_and_wall_free() {
        let grid = Grid::parse(DETOUR).expect("map should parse");
        let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Four);
        let path = outcome.path.expect("path exists");
        for (a, b) in path.iter().zip(path.iter().skip(1)) {
            assert_eq!(a.x.abs_diff(b.x) + a.y.abs_diff(b.y), 1, "not adjacent");
        }
        assert!(path.iter().all(|&p| grid.is_open(p)), "path crosses a wall");
    }

    #[test]
    fn first_step_enqueues_the_starts_open_neighbours() {
        let grid = Grid::parse(DETOUR).expect("map should parse");
        let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Four);
        let first = outcome.steps.first().expect("at least one step");
        let mut enqueued = first.enqueued.clone();
        enqueued.sort_by_key(|p| (p.y, p.x));
        let mut expected = grid.neighbors(grid.start(), Connectivity::Four);
        expected.sort_by_key(|p| (p.y, p.x));
        assert_eq!(enqueued, expected);
    }

    #[test]
    fn eight_connectivity_uses_diagonal_step_cost() {
        let grid = Grid::parse("S.\n.G\n").expect("map should parse");
        let outcome = search(&grid, &Euclidean, Algorithm::AStar, Connectivity::Eight);
        let cost = outcome.stats.path_cost.expect("path exists").value();
        assert!(
            (cost - std::f64::consts::SQRT_2).abs() < 1e-9,
            "cost was {cost}"
        );
    }

    #[test]
    fn astar_manhattan_eight_conn_still_reaches_goal() {
        // Inadmissible pairing (Manhattan on 8-connectivity) still finds a path.
        let grid = Grid::parse("S....\n.....\n.....\n.....\n....G\n").expect("map parses");
        let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Eight);
        assert!(outcome.path.is_some());
    }

    #[test]
    fn draining_the_open_set_skips_stale_duplicates() {
        // Large open region with the goal walled off on all three of its 8-way
        // sides: A* on 8-connectivity re-enqueues nodes, and draining the heap
        // pops the now-stale copies (the lazy-deletion skip).
        let grid = Grid::parse("S....\n.....\n.....\n...##\n...#G\n").expect("map parses");
        let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Eight);
        assert!(outcome.path.is_none());
    }

    #[test]
    fn dijkstra_ignores_the_heuristic() {
        // With Zero heuristic, A* and Dijkstra are identical strategies.
        let grid = Grid::parse(DETOUR).expect("map should parse");
        let a = search(&grid, &Zero, Algorithm::AStar, Connectivity::Four);
        let d = search(&grid, &Zero, Algorithm::Dijkstra, Connectivity::Four);
        assert_eq!(a.stats.path_cost, d.stats.path_cost);
    }

    #[test]
    fn frontier_pops_lowest_priority_first() {
        let mut heap = BinaryHeap::new();
        heap.push(frontier(5.0, 0.0, 0));
        heap.push(frontier(2.0, 0.0, 1));
        heap.push(frontier(3.0, 0.0, 2));
        assert_eq!(heap.pop().map(|f| f.priority), Some(Cost::new(2.0)));
    }

    #[test]
    fn frontier_tie_breaks_on_higher_g_then_fifo() {
        let base = frontier(2.0, 1.0, 0);
        assert!(frontier(2.0, 3.0, 1) > base); // higher g wins ties
        assert_eq!(
            base.partial_cmp(&frontier(2.0, 3.0, 1)),
            Some(Ordering::Less)
        );
        assert!(base > frontier(2.0, 1.0, 9)); // equal priority+g -> earlier seq
        assert_ne!(base, frontier(2.0, 1.0, 9));
        assert_eq!(base, frontier(2.0, 1.0, 0)); // ordering ignores pos
    }
}
