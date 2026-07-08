//! Terminal rendering of a search: animated frames and a text summary.
//!
//! Rendering is a pure replay of a [`SearchOutcome`]. Output goes to any
//! [`Write`], and the frame delay is a parameter, so animation is fully
//! testable by writing to a buffer with a zero delay. `animate` owns the
//! screen-clearing and flushing; [`render_solution`] draws in place so a
//! one-shot render never wipes the caller's terminal.

use std::collections::HashSet;
use std::io::{self, Write};
use std::time::Duration;

use crate::grid::{GOAL, Grid, OPEN, Pos, START, WALL};
use crate::search::SearchOutcome;

/// ANSI escape to clear the screen and move the cursor home.
const CLEAR: &[u8] = b"\x1b[2J\x1b[H";
/// ANSI escape resetting all styling.
const RESET: &str = "\x1b[0m";
/// Glyph for a cell on the final path.
const PATH: char = '*';
/// Glyph for the cell being expanded this frame.
const CURRENT: char = '@';
/// Glyph for a cell in the open set (frontier).
const FRONTIER: char = 'o';
/// Glyph for an already-expanded cell.
const VISITED: char = ':';

/// ANSI colour for the start cell.
const BLUE: &str = "\x1b[94m";
/// ANSI colour for the goal cell.
const MAGENTA: &str = "\x1b[95m";
/// ANSI colour for the path.
const GREEN: &str = "\x1b[92m";
/// ANSI colour for the current cell.
const YELLOW: &str = "\x1b[93m";
/// ANSI colour for frontier cells.
const CYAN: &str = "\x1b[96m";
/// ANSI colour for visited cells.
const GREY: &str = "\x1b[90m";
/// ANSI colour for walls.
const WHITE: &str = "\x1b[37m";

/// The overlay layer a cell belongs to, in precedence order (highest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    /// The start cell.
    Start,
    /// The goal cell.
    Goal,
    /// A cell on the final path.
    Path,
    /// The cell being expanded this frame.
    Current,
    /// A cell in the open set.
    Frontier,
    /// An already-expanded cell.
    Visited,
    /// An open, unremarkable cell.
    Open,
    /// A wall cell.
    Wall,
}

impl Layer {
    /// The glyph drawn for this layer.
    fn glyph(self) -> char {
        match self {
            Self::Start => START,
            Self::Goal => GOAL,
            Self::Path => PATH,
            Self::Current => CURRENT,
            Self::Frontier => FRONTIER,
            Self::Visited => VISITED,
            Self::Open => OPEN,
            Self::Wall => WALL,
        }
    }

    /// The ANSI colour escape for this layer (empty for open cells).
    fn color(self) -> &'static str {
        match self {
            Self::Start => BLUE,
            Self::Goal => MAGENTA,
            Self::Path => GREEN,
            Self::Current => YELLOW,
            Self::Frontier => CYAN,
            Self::Visited => GREY,
            Self::Wall => WHITE,
            Self::Open => "",
        }
    }
}

/// A borrowed view of the search state to overlay on the grid for one frame.
struct Overlay<'a> {
    /// The cell expanded this frame, if any.
    current: Option<Pos>,
    /// Cells currently in the open set.
    frontier: &'a HashSet<Pos>,
    /// Cells already expanded.
    visited: &'a HashSet<Pos>,
    /// Cells on the final path (empty until the search resolves).
    path: &'a [Pos],
}

/// Animate the search by replaying its trace, pausing `delay` between frames.
///
/// Clears the screen and flushes before each pause so the animation is visible
/// even through a buffered writer. When `color` is false, ANSI colour codes are
/// omitted (for non-terminal output).
///
/// # Errors
/// Propagates any [`io::Error`] from writing to `out`.
pub fn animate(
    grid: &Grid,
    outcome: &SearchOutcome,
    out: &mut dyn Write,
    delay: Duration,
    color: bool,
) -> io::Result<()> {
    let mut visited = HashSet::new();
    let mut frontier = HashSet::new();
    for step in &outcome.steps {
        visited.insert(step.expanded);
        frontier.remove(&step.expanded);
        frontier.extend(step.enqueued.iter().copied());
        draw_step(grid, step.expanded, &frontier, &visited, out, color)?;
        pause(delay);
    }
    out.write_all(CLEAR)?;
    render_solution(grid, outcome, out, color)?;
    out.flush()
}

/// Render the final frame in place, overlaying the solution path (if any).
///
/// Does not clear the screen, so a one-shot (non-animated) render leaves the
/// caller's scrollback intact.
///
/// # Errors
/// Propagates any [`io::Error`] from writing to `out`.
pub fn render_solution(
    grid: &Grid,
    outcome: &SearchOutcome,
    out: &mut dyn Write,
    color: bool,
) -> io::Result<()> {
    let empty = HashSet::new();
    let overlay = Overlay {
        current: None,
        frontier: &empty,
        visited: &empty,
        path: outcome.path.as_deref().unwrap_or(&[]),
    };
    render_frame(grid, &overlay, out, color)
}

/// Write a one-line-per-metric summary of the search.
///
/// Emits four lines — `expanded:`, `max frontier:`, `path cost:` (or
/// `(no path)` when unsolved), and `elapsed:`.
///
/// # Errors
/// Propagates any [`io::Error`] from writing to `out`.
pub fn summary(outcome: &SearchOutcome, elapsed: Duration, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "expanded:     {}", outcome.stats.expanded)?;
    writeln!(out, "max frontier: {}", outcome.stats.max_frontier)?;
    match outcome.stats.path_cost {
        Some(cost) => writeln!(out, "path cost:    {cost}")?,
        None => writeln!(out, "path cost:    (no path)")?,
    }
    writeln!(out, "elapsed:      {elapsed:?}")
}

/// Clear, draw one animation frame for `current`, and flush.
///
/// # Errors
/// Propagates any [`io::Error`] from writing to `out`.
fn draw_step(
    grid: &Grid,
    current: Pos,
    frontier: &HashSet<Pos>,
    visited: &HashSet<Pos>,
    out: &mut dyn Write,
    color: bool,
) -> io::Result<()> {
    out.write_all(CLEAR)?;
    let overlay = Overlay {
        current: Some(current),
        frontier,
        visited,
        path: &[],
    };
    render_frame(grid, &overlay, out, color)?;
    out.flush()
}

/// Draw one overlaid frame of the grid (no screen clear).
///
/// # Errors
/// Propagates any [`io::Error`] from writing to `out`.
fn render_frame(
    grid: &Grid,
    overlay: &Overlay,
    out: &mut dyn Write,
    color: bool,
) -> io::Result<()> {
    for y in 0..grid.height() {
        render_row(grid, overlay, y, out, color)?;
    }
    Ok(())
}

/// Draw row `y` of the grid, one styled glyph per column. Colour escapes are
/// emitted only when `color` is true.
///
/// # Errors
/// Propagates any [`io::Error`] from writing to `out`.
fn render_row(
    grid: &Grid,
    overlay: &Overlay,
    y: usize,
    out: &mut dyn Write,
    color: bool,
) -> io::Result<()> {
    for x in 0..grid.width() {
        let layer = overlay_cell(grid, overlay, Pos::new(x, y));
        let (pre, post) = if color {
            (layer.color(), RESET)
        } else {
            ("", "")
        };
        write!(out, "{pre}{}{post}", layer.glyph())?;
    }
    out.write_all(b"\n")
}

/// The [`Layer`] for `pos`, applying precedence: start/goal, then path,
/// current, frontier, visited, and finally the grid's own open/wall cell.
fn overlay_cell(grid: &Grid, overlay: &Overlay, pos: Pos) -> Layer {
    if pos == grid.start() {
        Layer::Start
    } else if pos == grid.goal() {
        Layer::Goal
    } else if overlay.path.contains(&pos) {
        Layer::Path
    } else if overlay.current == Some(pos) {
        Layer::Current
    } else if overlay.frontier.contains(&pos) {
        Layer::Frontier
    } else if overlay.visited.contains(&pos) {
        Layer::Visited
    } else if grid.is_open(pos) {
        Layer::Open
    } else {
        Layer::Wall
    }
}

/// Sleep for `delay` unless it is zero (tests pass zero to stay fast).
fn pause(delay: Duration) {
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::grid::Connectivity;
    use crate::heuristic::Manhattan;
    use crate::search::{Algorithm, search};

    /// The map used across the render tests (wall at (1,1)).
    const MAP: &str = "S..\n.#.\n..G\n";

    fn solved() -> (Grid, SearchOutcome) {
        let grid = Grid::parse(MAP).expect("map parses");
        let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Four);
        (grid, outcome)
    }

    #[test]
    fn overlay_cell_applies_precedence() {
        let grid = Grid::parse(MAP).expect("map parses");
        let visited: HashSet<Pos> = [Pos::new(1, 0)].into_iter().collect();
        let frontier: HashSet<Pos> = [Pos::new(0, 1)].into_iter().collect();
        let path = [Pos::new(2, 0)];
        let overlay = Overlay {
            current: Some(Pos::new(0, 2)),
            frontier: &frontier,
            visited: &visited,
            path: &path,
        };
        let glyph = |pos| overlay_cell(&grid, &overlay, pos).glyph();
        assert_eq!(glyph(grid.start()), START);
        assert_eq!(glyph(grid.goal()), GOAL);
        assert_eq!(glyph(Pos::new(2, 0)), PATH);
        assert_eq!(glyph(Pos::new(0, 2)), CURRENT);
        assert_eq!(glyph(Pos::new(0, 1)), FRONTIER);
        assert_eq!(glyph(Pos::new(1, 0)), VISITED);
        assert_eq!(glyph(Pos::new(1, 1)), WALL);
        assert_eq!(glyph(Pos::new(2, 1)), OPEN);
    }

    #[test]
    fn overlay_cell_precedence_resolves_overlaps() {
        let grid = Grid::parse(MAP).expect("map parses");
        // Cells deliberately belonging to several categories at once.
        let frontier: HashSet<Pos> = [Pos::new(0, 1), Pos::new(2, 0)].into_iter().collect();
        let visited: HashSet<Pos> = [Pos::new(0, 1), Pos::new(2, 0), Pos::new(1, 0)]
            .into_iter()
            .collect();
        let path = [Pos::new(1, 0)];
        let overlay = Overlay {
            current: Some(Pos::new(0, 1)),
            frontier: &frontier,
            visited: &visited,
            path: &path,
        };
        let glyph = |pos| overlay_cell(&grid, &overlay, pos).glyph();
        assert_eq!(glyph(Pos::new(1, 0)), PATH); // path outranks visited
        assert_eq!(glyph(Pos::new(0, 1)), CURRENT); // current outranks frontier/visited
        assert_eq!(glyph(Pos::new(2, 0)), FRONTIER); // frontier outranks visited
    }

    #[test]
    fn every_layer_has_glyph_and_color() {
        let coloured = [
            Layer::Start,
            Layer::Goal,
            Layer::Path,
            Layer::Current,
            Layer::Frontier,
            Layer::Visited,
            Layer::Wall,
        ];
        for layer in coloured {
            assert!(!layer.color().is_empty(), "{layer:?} should be coloured");
        }
        assert_eq!(Layer::Open.color(), "");
        assert_eq!(Layer::Open.glyph(), OPEN);
    }

    #[test]
    fn render_frame_has_one_line_per_row() {
        let (grid, _) = solved();
        let empty = HashSet::new();
        let overlay = Overlay {
            current: None,
            frontier: &empty,
            visited: &empty,
            path: &[],
        };
        let mut buf = Vec::new();
        render_frame(&grid, &overlay, &mut buf, true).expect("write ok");
        let frame = String::from_utf8(buf).expect("utf-8");
        assert_eq!(frame.matches('\n').count(), grid.height());
        assert!(frame.contains(START) && frame.contains(GOAL) && frame.contains(WALL));
    }

    #[test]
    fn render_frame_without_color_omits_escapes() {
        let (grid, _) = solved();
        let empty = HashSet::new();
        let overlay = Overlay {
            current: None,
            frontier: &empty,
            visited: &empty,
            path: &[],
        };
        let mut buf = Vec::new();
        render_frame(&grid, &overlay, &mut buf, false).expect("write ok");
        let frame = String::from_utf8(buf).expect("utf-8");
        assert!(!frame.contains('\x1b'), "no ANSI escapes without colour");
        assert!(frame.contains(START) && frame.contains(GOAL));
    }

    #[test]
    fn animate_emits_frames_and_final_path() {
        let (grid, outcome) = solved();
        let mut buf = Vec::new();
        animate(&grid, &outcome, &mut buf, Duration::ZERO, true).expect("write ok");
        let out = String::from_utf8(buf).expect("utf-8");
        assert!(out.contains(CURRENT), "animation shows the expanding cell");
        assert!(out.contains(PATH), "final frame shows the path");
    }

    #[test]
    fn animate_shows_expanded_cells_as_visited() {
        // Once expanded, a cell must leave the frontier and render as VISITED.
        let (grid, outcome) = solved();
        let mut buf = Vec::new();
        animate(&grid, &outcome, &mut buf, Duration::ZERO, true).expect("write ok");
        assert!(
            String::from_utf8(buf).expect("utf-8").contains(VISITED),
            "expanded cells should render as visited"
        );
    }

    #[test]
    fn animate_with_nonzero_delay_runs() {
        // Exercises the pause() sleep branch with a negligible delay.
        let (grid, outcome) = solved();
        let mut buf = Vec::new();
        animate(&grid, &outcome, &mut buf, Duration::from_nanos(1), true).expect("write ok");
        assert!(!buf.is_empty());
    }

    #[test]
    fn summary_prints_the_actual_stat_values() {
        let (_, outcome) = solved();
        let mut buf = Vec::new();
        summary(&outcome, Duration::from_millis(3), &mut buf).expect("write ok");
        let out = String::from_utf8(buf).expect("utf-8");
        assert!(out.contains(&format!("expanded:     {}", outcome.stats.expanded)));
        assert!(out.contains(&format!("max frontier: {}", outcome.stats.max_frontier)));
        let cost = outcome.stats.path_cost.expect("solved grid has a path");
        assert!(out.contains(&format!("path cost:    {cost}")));
    }

    #[test]
    fn summary_marks_missing_path() {
        let grid = Grid::parse("S#G\n").expect("map parses");
        let outcome = search(&grid, &Manhattan, Algorithm::AStar, Connectivity::Four);
        let mut buf = Vec::new();
        summary(&outcome, Duration::ZERO, &mut buf).expect("write ok");
        assert!(String::from_utf8(buf).expect("utf-8").contains("(no path)"));
    }
}
