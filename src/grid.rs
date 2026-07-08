//! The 2D grid model: positions, cells, connectivity, and parsing.

use std::fmt;
use std::fmt::Write as _;

/// Glyph for an open, walkable cell.
const OPEN: char = '.';
/// Glyph for a wall cell.
const WALL: char = '#';
/// Glyph marking the start position.
const START: char = 'S';
/// Glyph marking the goal position.
const GOAL: char = 'G';

/// A cell coordinate: `x` is the column (left to right), `y` the row (top to
/// bottom), both zero-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos {
    /// Column index.
    pub x: usize,
    /// Row index.
    pub y: usize,
}

impl Pos {
    /// Create a position at column `x`, row `y`.
    #[must_use]
    pub const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

/// A single grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Open, walkable space.
    Open,
    /// An impassable wall.
    Wall,
}

/// Movement connectivity: which neighbours are reachable in one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    /// 4-directional movement (orthogonal only).
    Four,
    /// 8-directional movement (orthogonal plus diagonals).
    Eight,
}

impl Connectivity {
    /// The `(dx, dy)` neighbour offsets for this connectivity.
    fn offsets(self) -> &'static [(isize, isize)] {
        /// Orthogonal offsets: up, down, left, right.
        const FOUR: [(isize, isize); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
        /// Orthogonal offsets plus the four diagonals.
        const EIGHT: [(isize, isize); 8] = [
            (0, -1),
            (0, 1),
            (-1, 0),
            (1, 0),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ];
        match self {
            Self::Four => &FOUR,
            Self::Eight => &EIGHT,
        }
    }
}

/// An error produced while parsing a [`Grid`] from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    /// The input contained no rows.
    Empty,
    /// A row's width differed from the first row.
    RaggedRow {
        /// Zero-based index of the row whose width did not match.
        row: usize,
        /// The width established by the first row.
        expected: usize,
        /// The width actually found on this row.
        found: usize,
    },
    /// No start cell `S` was found.
    MissingStart,
    /// No goal cell `G` was found.
    MissingGoal,
    /// More than one start cell `S` was found.
    DuplicateStart,
    /// More than one goal cell `G` was found.
    DuplicateGoal,
    /// An unrecognised character was encountered.
    UnknownChar(char),
}

impl fmt::Display for MapError {
    /// Write a human-readable description of the error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("map is empty"),
            Self::RaggedRow {
                row,
                expected,
                found,
            } => write!(f, "row {row} has width {found}, expected {expected}"),
            Self::MissingStart => write!(f, "map has no start cell '{START}'"),
            Self::MissingGoal => write!(f, "map has no goal cell '{GOAL}'"),
            Self::DuplicateStart => write!(f, "map has more than one start cell '{START}'"),
            Self::DuplicateGoal => write!(f, "map has more than one goal cell '{GOAL}'"),
            Self::UnknownChar(ch) => write!(f, "unknown character '{ch}' in map"),
        }
    }
}

impl std::error::Error for MapError {}

/// A rectangular grid of cells with a designated start and goal.
#[derive(Debug, Clone)]
pub struct Grid {
    /// Row-major cells; the cell at `(x, y)` is index `y * width + x`.
    cells: Vec<Cell>,
    /// Number of columns.
    width: usize,
    /// Number of rows.
    height: usize,
    /// The start position.
    start: Pos,
    /// The goal position.
    goal: Pos,
}

impl Grid {
    /// Parse a grid from a multi-line string.
    ///
    /// Recognised characters: `.` open, `#` wall, `S` start, `G` goal. Every
    /// line is a grid row and all rows must share one width; blank lines are
    /// not permitted (they surface as a width mismatch).
    ///
    /// # Errors
    /// Returns a [`MapError`] if the input is empty, rows are ragged, an
    /// unknown character appears, or the start/goal is missing or duplicated.
    ///
    /// # Examples
    /// ```
    /// use example_project::grid::Grid;
    /// let grid = Grid::parse("S..\n.#.\n..G\n").unwrap();
    /// assert_eq!((grid.width(), grid.height()), (3, 3));
    /// ```
    pub fn parse(text: &str) -> Result<Self, MapError> {
        let rows: Vec<&str> = text.lines().collect();
        let width = rows.first().ok_or(MapError::Empty)?.chars().count();
        let mut builder = Builder::new(width);
        for (y, row) in rows.iter().enumerate() {
            builder.push_row(y, row)?;
        }
        builder.finish(rows.len())
    }

    /// Whether `pos` lies within the grid's bounds.
    #[must_use]
    pub const fn in_bounds(&self, pos: Pos) -> bool {
        pos.x < self.width && pos.y < self.height
    }

    /// The cell at `pos`, or `None` if `pos` is out of bounds.
    #[must_use]
    pub fn get(&self, pos: Pos) -> Option<Cell> {
        if !self.in_bounds(pos) {
            return None;
        }
        self.cells.get(pos.y * self.width + pos.x).copied()
    }

    /// Whether `pos` is in bounds and walkable (not a wall).
    #[must_use]
    pub fn is_open(&self, pos: Pos) -> bool {
        matches!(self.get(pos), Some(Cell::Open))
    }

    /// The in-bounds, walkable neighbours of `pos` under `conn`.
    ///
    /// Under [`Connectivity::Eight`] a diagonal neighbour is included whenever
    /// the target cell is open, so paths may cut around wall corners.
    #[must_use]
    pub fn neighbors(&self, pos: Pos, conn: Connectivity) -> Vec<Pos> {
        conn.offsets()
            .iter()
            .filter_map(|&(dx, dy)| self.offset(pos, dx, dy))
            .filter(|&next| self.is_open(next))
            .collect()
    }

    /// Apply signed offset `(dx, dy)` to `pos`, returning the target if it
    /// lands in bounds.
    fn offset(&self, pos: Pos, dx: isize, dy: isize) -> Option<Pos> {
        let x = pos.x.checked_add_signed(dx)?;
        let y = pos.y.checked_add_signed(dy)?;
        self.in_bounds(Pos::new(x, y)).then_some(Pos::new(x, y))
    }

    /// The glyph representing the cell at `pos`.
    fn glyph(&self, pos: Pos) -> char {
        if pos == self.start {
            START
        } else if pos == self.goal {
            GOAL
        } else if self.is_open(pos) {
            OPEN
        } else {
            WALL
        }
    }

    /// The grid width in columns.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// The grid height in rows.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// The start position.
    #[must_use]
    pub const fn start(&self) -> Pos {
        self.start
    }

    /// The goal position.
    #[must_use]
    pub const fn goal(&self) -> Pos {
        self.goal
    }
}

impl fmt::Display for Grid {
    /// Render the grid as text using the same glyphs [`Grid::parse`] accepts.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.height {
            for x in 0..self.width {
                f.write_char(self.glyph(Pos::new(x, y)))?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

/// Accumulates cells and the start/goal positions while parsing rows.
struct Builder {
    /// The expected width of every row.
    width: usize,
    /// Row-major cells collected so far.
    cells: Vec<Cell>,
    /// The start position, once a `S` has been seen.
    start: Option<Pos>,
    /// The goal position, once a `G` has been seen.
    goal: Option<Pos>,
}

impl Builder {
    /// Create a builder expecting rows of `width` columns.
    fn new(width: usize) -> Self {
        Self {
            width,
            cells: Vec::new(),
            start: None,
            goal: None,
        }
    }

    /// Parse one row at index `y`, appending its cells.
    ///
    /// # Errors
    /// [`MapError::RaggedRow`] if the row's width differs from the first row,
    /// or any error from [`Builder::push_char`].
    fn push_row(&mut self, y: usize, row: &str) -> Result<(), MapError> {
        let found = row.chars().count();
        if found != self.width {
            return Err(MapError::RaggedRow {
                row: y,
                expected: self.width,
                found,
            });
        }
        for (x, ch) in row.chars().enumerate() {
            self.push_char(Pos::new(x, y), ch)?;
        }
        Ok(())
    }

    /// Parse one character into a cell, tracking start and goal positions.
    ///
    /// # Errors
    /// [`MapError::UnknownChar`] for a character other than `.`, `#`, `S`, or
    /// `G`; [`MapError::DuplicateStart`] / [`MapError::DuplicateGoal`] if a
    /// second `S` or `G` is encountered.
    fn push_char(&mut self, pos: Pos, ch: char) -> Result<(), MapError> {
        let cell = match ch {
            OPEN => Cell::Open,
            WALL => Cell::Wall,
            START => Self::mark(&mut self.start, pos, MapError::DuplicateStart)?,
            GOAL => Self::mark(&mut self.goal, pos, MapError::DuplicateGoal)?,
            other => return Err(MapError::UnknownChar(other)),
        };
        self.cells.push(cell);
        Ok(())
    }

    /// Store `pos` into `slot`, rejecting a second occurrence.
    ///
    /// # Errors
    /// Returns `dup` if `slot` already holds a position.
    fn mark(slot: &mut Option<Pos>, pos: Pos, dup: MapError) -> Result<Cell, MapError> {
        if slot.is_some() {
            return Err(dup);
        }
        *slot = Some(pos);
        Ok(Cell::Open)
    }

    /// Finish building, validating that both start and goal were found.
    ///
    /// # Errors
    /// [`MapError::MissingStart`] or [`MapError::MissingGoal`].
    fn finish(self, height: usize) -> Result<Grid, MapError> {
        let start = self.start.ok_or(MapError::MissingStart)?;
        let goal = self.goal.ok_or(MapError::MissingGoal)?;
        Ok(Grid {
            cells: self.cells,
            width: self.width,
            height,
            start,
            goal,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A small valid map used across several tests.
    const MAP: &str = "S..\n.#.\n..G\n";

    fn parse_ok(text: &str) -> Grid {
        Grid::parse(text).expect("map should parse")
    }

    #[test]
    fn parse_reads_dimensions_and_endpoints() {
        let grid = parse_ok(MAP);
        assert_eq!(grid.width(), 3);
        assert_eq!(grid.height(), 3);
        assert_eq!(grid.start(), Pos::new(0, 0));
        assert_eq!(grid.goal(), Pos::new(2, 2));
    }

    #[test]
    fn parse_rejects_blank_interior_line() {
        // A blank line is a zero-width row, so it fails the width check.
        assert_eq!(
            Grid::parse("S..\n\n..G\n").unwrap_err(),
            MapError::RaggedRow {
                row: 1,
                expected: 3,
                found: 0,
            }
        );
    }

    #[test]
    fn parse_marks_walls_and_open_cells() {
        let grid = parse_ok(MAP);
        assert_eq!(grid.get(Pos::new(1, 1)), Some(Cell::Wall));
        assert!(grid.is_open(Pos::new(0, 0)));
        assert!(!grid.is_open(Pos::new(1, 1)));
    }

    #[test]
    fn get_and_is_open_reject_out_of_bounds() {
        let grid = parse_ok(MAP);
        assert_eq!(grid.get(Pos::new(3, 0)), None);
        assert_eq!(grid.get(Pos::new(0, 3)), None);
        assert!(!grid.is_open(Pos::new(9, 9)));
    }

    #[test]
    fn parse_empty_input_errors() {
        assert_eq!(Grid::parse("").unwrap_err(), MapError::Empty);
    }

    #[test]
    fn parse_ragged_rows_error() {
        assert_eq!(
            Grid::parse("S..\n.G\n").unwrap_err(),
            MapError::RaggedRow {
                row: 1,
                expected: 3,
                found: 2,
            }
        );
    }

    #[test]
    fn parse_missing_start_or_goal_errors() {
        assert_eq!(
            Grid::parse("...\n..G\n").unwrap_err(),
            MapError::MissingStart
        );
        assert_eq!(
            Grid::parse("S..\n...\n").unwrap_err(),
            MapError::MissingGoal
        );
    }

    #[test]
    fn parse_duplicate_start_or_goal_errors() {
        assert_eq!(
            Grid::parse("S.S\n..G\n").unwrap_err(),
            MapError::DuplicateStart
        );
        assert_eq!(
            Grid::parse("S.G\n..G\n").unwrap_err(),
            MapError::DuplicateGoal
        );
    }

    #[test]
    fn parse_unknown_char_errors() {
        assert_eq!(
            Grid::parse("S.?\n..G\n").unwrap_err(),
            MapError::UnknownChar('?')
        );
    }

    #[test]
    fn neighbors_four_at_corner_excludes_off_grid_cells() {
        let grid = parse_ok(MAP);
        let mut got = grid.neighbors(Pos::new(0, 0), Connectivity::Four);
        got.sort_by_key(|p| (p.y, p.x));
        // (0,0) corner: up/left underflow off the grid; right and down are open.
        // (The wall at (1,1) is diagonal, so it is never a Four candidate here.)
        assert_eq!(got, vec![Pos::new(1, 0), Pos::new(0, 1)]);
    }

    #[test]
    fn neighbors_four_excludes_in_bounds_wall() {
        let grid = parse_ok(MAP); // wall at (1,1)
        let mut got = grid.neighbors(Pos::new(1, 0), Connectivity::Four);
        got.sort_by_key(|p| (p.y, p.x));
        // Down (1,1) is in bounds but a wall -> dropped; up is off the edge.
        assert_eq!(got, vec![Pos::new(0, 0), Pos::new(2, 0)]);
        assert!(grid.in_bounds(Pos::new(1, 1)));
        assert!(!got.contains(&Pos::new(1, 1)));
    }

    #[test]
    fn neighbors_four_excludes_far_edge() {
        let grid = parse_ok(MAP); // 3x3, evaluate the bottom-right corner
        let mut got = grid.neighbors(Pos::new(2, 2), Connectivity::Four);
        got.sort_by_key(|p| (p.y, p.x));
        // right (3,2) and down (2,3) fail offset's in_bounds guard (not
        // underflow), leaving up (2,1) and left (1,2).
        assert_eq!(got, vec![Pos::new(2, 1), Pos::new(1, 2)]);
    }

    #[test]
    fn neighbors_eight_allows_corner_cutting() {
        // (1,0) and (0,1) are walls flanking the open diagonal (1,1).
        let grid = parse_ok("S#.\n#..\n..G\n");
        let got = grid.neighbors(Pos::new(0, 0), Connectivity::Eight);
        // Diagonal reachable even though both flanking orthogonals are walls.
        assert_eq!(got, vec![Pos::new(1, 1)]);
    }

    #[test]
    fn in_bounds_inside_true_past_edges_false() {
        let grid = parse_ok(MAP); // 3x3
        assert!(grid.in_bounds(Pos::new(0, 0)));
        assert!(grid.in_bounds(Pos::new(2, 2)));
        assert!(!grid.in_bounds(Pos::new(3, 2))); // x == width
        assert!(!grid.in_bounds(Pos::new(2, 3))); // y == height
    }

    #[test]
    fn neighbors_eight_includes_open_diagonals() {
        // Goal sits at (3,2), clear of (1,1)'s eight surrounding cells.
        let grid = parse_ok("....\n.S..\n...G\n");
        let got = grid.neighbors(Pos::new(1, 1), Connectivity::Eight);
        assert_eq!(got.len(), 8);
    }

    #[test]
    fn display_round_trips_glyphs() {
        let grid = parse_ok(MAP);
        assert_eq!(grid.to_string(), MAP);
    }

    #[test]
    fn map_error_display_is_non_empty_for_every_variant() {
        let variants = [
            MapError::Empty,
            MapError::RaggedRow {
                row: 2,
                expected: 5,
                found: 3,
            },
            MapError::MissingStart,
            MapError::MissingGoal,
            MapError::DuplicateStart,
            MapError::DuplicateGoal,
            MapError::UnknownChar('x'),
        ];
        for err in &variants {
            assert!(!err.to_string().is_empty());
        }
    }

    #[test]
    fn map_error_display_formats_details() {
        let ragged = MapError::RaggedRow {
            row: 2,
            expected: 5,
            found: 3,
        };
        assert_eq!(ragged.to_string(), "row 2 has width 3, expected 5");
        let unknown = MapError::UnknownChar('x');
        assert_eq!(unknown.to_string(), "unknown character 'x' in map");
    }
}
