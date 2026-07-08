//! Command-line interface: parse arguments, resolve a map source, run and
//! render the search.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::{Args, Parser, ValueEnum};

use crate::examples;
use crate::grid::{Connectivity, Grid, MapError};
use crate::heuristic::{Heuristic, HeuristicKind};
use crate::mapgen::MapSpec;
use crate::render::{animate, render_solution, summary};
use crate::search::{Algorithm, SearchOutcome, search};

/// Command-line arguments for the A\* search simulator.
#[derive(Debug, Parser)]
#[command(
    version,
    about = "A* search simulator: compare A* and greedy with swappable heuristics."
)]
pub struct Cli {
    /// Where the map comes from.
    #[command(flatten)]
    source: Source,
    /// Search algorithm (ignored with `--compare`).
    #[arg(long, value_enum, default_value_t = AlgoArg::Astar)]
    algo: AlgoArg,
    /// Heuristic used to estimate the remaining cost.
    #[arg(long, value_enum, default_value_t = HeuristicArg::Manhattan)]
    heuristic: HeuristicArg,
    /// Movement connectivity: 4-directional or 8-directional (diagonals).
    #[arg(long, value_enum, default_value_t = ConnArg::Four)]
    connectivity: ConnArg,
    /// Run A\* and greedy back to back and show both summaries.
    #[arg(long)]
    compare: bool,
    /// Print a one-shot summary instead of animating.
    #[arg(long)]
    summary: bool,
    /// Disable ANSI colour (for piping to a file or non-terminal).
    #[arg(long)]
    no_color: bool,
    /// Milliseconds to pause between animation frames.
    #[arg(long, default_value_t = 80)]
    delay: u64,
}

/// The map source: a file, a built-in example, or a random map. At most one of
/// `--map`, `--example`, `--random` may be given; the default is example
/// `open`.
#[derive(Debug, Args)]
struct Source {
    /// Load a map from an ASCII file.
    #[arg(long, value_name = "FILE", group = "source")]
    map: Option<PathBuf>,
    /// Use a built-in example map by name; also the default source when none
    /// of --map/--example/--random is given (example "open").
    #[arg(long, value_name = "NAME", group = "source")]
    example: Option<String>,
    /// Generate a random map.
    #[arg(long, group = "source")]
    random: bool,
    /// Width of a random map.
    #[arg(long, default_value_t = 24)]
    width: usize,
    /// Height of a random map.
    #[arg(long, default_value_t = 12)]
    height: usize,
    /// Wall density of a random map, in `0.0..=1.0`.
    #[arg(long, default_value_t = 0.3)]
    density: f64,
    /// Seed for a random map.
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

/// CLI selector for the search algorithm.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum AlgoArg {
    /// A\* (`f = g + h`).
    Astar,
    /// Greedy best-first (`h` only).
    Greedy,
    /// Dijkstra (`g` only).
    Dijkstra,
}

impl AlgoArg {
    /// The core [`Algorithm`] this selector names.
    fn to_core(self) -> Algorithm {
        match self {
            Self::Astar => Algorithm::AStar,
            Self::Greedy => Algorithm::Greedy,
            Self::Dijkstra => Algorithm::Dijkstra,
        }
    }
}

/// CLI selector for the heuristic.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum HeuristicArg {
    /// Manhattan distance.
    Manhattan,
    /// Euclidean distance.
    Euclidean,
    /// Chebyshev distance.
    Chebyshev,
    /// The zero heuristic (turns A\* into Dijkstra).
    Zero,
}

impl HeuristicArg {
    /// The core [`HeuristicKind`] this selector names.
    fn to_core(self) -> HeuristicKind {
        match self {
            Self::Manhattan => HeuristicKind::Manhattan,
            Self::Euclidean => HeuristicKind::Euclidean,
            Self::Chebyshev => HeuristicKind::Chebyshev,
            Self::Zero => HeuristicKind::Zero,
        }
    }
}

/// CLI selector for movement connectivity.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum ConnArg {
    /// 4-directional movement (orthogonal only).
    Four,
    /// 8-directional movement (orthogonal plus diagonals).
    Eight,
}

impl ConnArg {
    /// The core [`Connectivity`] this selector names.
    fn to_core(self) -> Connectivity {
        match self {
            Self::Four => Connectivity::Four,
            Self::Eight => Connectivity::Eight,
        }
    }
}

/// An error while running the CLI.
#[derive(Debug)]
pub enum CliError {
    /// A map file could not be read.
    ReadFile {
        /// The path that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// A built-in example of this name does not exist.
    UnknownExample(String),
    /// The map text was invalid.
    Map(MapError),
    /// Writing output failed.
    Io(io::Error),
}

impl fmt::Display for CliError {
    /// Write a human-readable description of the error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(f, "cannot read map file {}: {source}", path.display())
            }
            Self::UnknownExample(name) => write!(f, "unknown example '{name}'"),
            Self::Map(err) => write!(f, "invalid map: {err}"),
            Self::Io(err) => write!(f, "output error: {err}"),
        }
    }
}

impl std::error::Error for CliError {
    /// The underlying cause, for callers that walk the error chain.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::Map(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::UnknownExample(_) => None,
        }
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<MapError> for CliError {
    fn from(err: MapError) -> Self {
        Self::Map(err)
    }
}

/// Run the simulator described by `cli`, writing all output to `out`.
///
/// # Errors
/// Returns a [`CliError`] on a bad map source, an invalid map, or I/O failure.
pub fn run(cli: &Cli, out: &mut dyn Write) -> Result<(), CliError> {
    let grid = cli.source.load()?;
    let heuristic = cli.heuristic.to_core().build();
    let conn = cli.connectivity.to_core();
    let color = !cli.no_color;
    if cli.compare {
        run_compare(&grid, heuristic.as_ref(), conn, color, out)
    } else {
        run_single(&grid, heuristic.as_ref(), cli, conn, color, out)
    }
}

/// Run one search and either animate it or print its final state.
///
/// # Errors
/// Propagates any output [`CliError`].
fn run_single(
    grid: &Grid,
    heuristic: &dyn Heuristic,
    cli: &Cli,
    conn: Connectivity,
    color: bool,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let (outcome, elapsed) = timed_search(grid, heuristic, cli.algo.to_core(), conn);
    if cli.summary {
        render_solution(grid, &outcome, out, color)?;
    } else {
        animate(grid, &outcome, out, Duration::from_millis(cli.delay), color)?;
    }
    summary(&outcome, elapsed, out)?;
    Ok(())
}

/// Run A\* and greedy on the same grid, printing each labelled summary.
///
/// # Errors
/// Propagates any output [`CliError`].
fn run_compare(
    grid: &Grid,
    heuristic: &dyn Heuristic,
    conn: Connectivity,
    color: bool,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    for algo in [Algorithm::AStar, Algorithm::Greedy] {
        writeln!(out, "== {algo} ==")?;
        let (outcome, elapsed) = timed_search(grid, heuristic, algo, conn);
        render_solution(grid, &outcome, out, color)?;
        summary(&outcome, elapsed, out)?;
    }
    Ok(())
}

/// Run a search and measure how long it took.
fn timed_search(
    grid: &Grid,
    heuristic: &dyn Heuristic,
    algo: Algorithm,
    conn: Connectivity,
) -> (SearchOutcome, Duration) {
    let start = Instant::now();
    let outcome = search(grid, heuristic, algo, conn);
    (outcome, start.elapsed())
}

impl Source {
    /// Resolve this source into a [`Grid`].
    ///
    /// # Errors
    /// Returns a [`CliError`] if a map file cannot be read, an example name is
    /// unknown, the random-map dimensions are too small for a distinct start
    /// and goal, or the resulting map text is invalid.
    fn load(&self) -> Result<Grid, CliError> {
        if let Some(path) = &self.map {
            return self.load_file(path);
        }
        if self.random {
            return Ok(self.spec().generate()?);
        }
        let name = self.example.as_deref().unwrap_or(examples::DEFAULT);
        let text =
            examples::example(name).ok_or_else(|| CliError::UnknownExample(name.to_owned()))?;
        Ok(Grid::parse(text)?)
    }

    /// Read and parse a map file.
    ///
    /// # Errors
    /// [`CliError::ReadFile`] on an I/O failure, or [`CliError::Map`] if the
    /// contents do not parse.
    fn load_file(&self, path: &Path) -> Result<Grid, CliError> {
        let text = fs::read_to_string(path).map_err(|source| CliError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Grid::parse(&text)?)
    }

    /// The [`MapSpec`] described by the random-map arguments.
    fn spec(&self) -> MapSpec {
        MapSpec {
            width: self.width,
            height: self.height,
            wall_density: self.density,
            seed: self.seed,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A writer whose every operation fails, to exercise the I/O error path.
    struct FailWriter;

    impl Write for FailWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("write failed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failed"))
        }
    }

    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["astar"];
        full.extend_from_slice(args);
        Cli::parse_from(full)
    }

    fn run_to_string(args: &[&str]) -> String {
        let cli = parse(args);
        let mut buf = Vec::new();
        run(&cli, &mut buf).expect("run should succeed");
        String::from_utf8(buf).expect("utf-8")
    }

    /// Write `contents` to a uniquely-named temp file and return its path.
    fn temp_map(tag: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("astar_{tag}_{}.txt", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        path
    }

    /// Drop lines that vary between runs (timing) so outputs can be compared.
    fn without_elapsed(text: String) -> String {
        text.lines()
            .filter(|line| !line.starts_with("elapsed:"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn single_summary_reports_a_path() {
        let out = run_to_string(&["--example", "open", "--summary"]);
        assert!(out.contains("expanded:"));
        assert!(out.contains("path cost:"));
    }

    #[test]
    fn single_animation_shows_the_current_cell() {
        let out = run_to_string(&["--example", "open", "--delay", "0"]);
        assert!(out.contains('@'));
    }

    #[test]
    fn compare_runs_both_algorithms() {
        let out = run_to_string(&["--example", "rooms", "--compare"]);
        assert!(out.contains("== A* =="));
        assert!(out.contains("== greedy =="));
    }

    #[test]
    fn random_source_is_seeded_and_runs() {
        let out = run_to_string(&[
            "--random",
            "--width",
            "8",
            "--height",
            "6",
            "--seed",
            "3",
            "--summary",
        ]);
        assert!(out.contains("expanded:"));
    }

    #[test]
    fn heuristics_and_algorithms_all_dispatch() {
        for heuristic in ["manhattan", "euclidean", "chebyshev", "zero"] {
            let out = run_to_string(&["--example", "open", "--summary", "--heuristic", heuristic]);
            assert!(out.contains("expanded:"));
        }
        for algo in ["astar", "greedy", "dijkstra"] {
            let out = run_to_string(&["--example", "open", "--summary", "--algo", algo]);
            assert!(out.contains("expanded:"));
        }
    }

    #[test]
    fn file_source_loads_and_runs() {
        let path = temp_map("ok", "S..\n.#.\n..G\n");
        let out = run_to_string(&["--map", path.to_str().unwrap(), "--summary"]);
        std::fs::remove_file(&path).ok();
        assert!(out.contains("path cost:"));
    }

    #[test]
    fn missing_file_is_a_read_error() {
        let cli = parse(&["--map", "/no/such/astar/map.txt", "--summary"]);
        let err = run(&cli, &mut Vec::new()).unwrap_err();
        assert!(matches!(err, CliError::ReadFile { .. }));
    }

    #[test]
    fn unknown_example_is_an_error() {
        let cli = parse(&["--example", "nope", "--summary"]);
        let err = run(&cli, &mut Vec::new()).unwrap_err();
        assert!(matches!(err, CliError::UnknownExample(_)));
    }

    #[test]
    fn invalid_map_file_is_a_map_error() {
        let path = temp_map("bad", "S..\n...\n"); // no goal
        let cli = parse(&["--map", path.to_str().unwrap(), "--summary"]);
        let err = run(&cli, &mut Vec::new()).unwrap_err();
        std::fs::remove_file(&path).ok();
        assert!(matches!(err, CliError::Map(MapError::MissingGoal)));
    }

    #[test]
    fn output_failure_is_an_io_error() {
        let cli = parse(&["--example", "open", "--summary"]);
        let err = run(&cli, &mut FailWriter).unwrap_err();
        assert!(matches!(err, CliError::Io(_)));
        // flush() is part of the Write contract but unreached above (write
        // fails first), so exercise it directly.
        let mut writer = FailWriter;
        assert!(writer.flush().is_err());
    }

    #[test]
    fn cli_error_display_is_populated_for_each_variant() {
        let errors = [
            CliError::ReadFile {
                path: PathBuf::from("m.txt"),
                source: io::Error::other("x"),
            },
            CliError::UnknownExample("q".to_owned()),
            CliError::Map(MapError::Empty),
            CliError::Io(io::Error::other("y")),
        ];
        for err in &errors {
            assert!(!err.to_string().is_empty());
        }
    }

    #[test]
    fn cli_error_source_chains_the_cause() {
        use std::error::Error;
        let read = CliError::ReadFile {
            path: PathBuf::from("m.txt"),
            source: io::Error::other("x"),
        };
        assert!(read.source().is_some());
        assert!(CliError::Map(MapError::Empty).source().is_some());
        assert!(CliError::Io(io::Error::other("y")).source().is_some());
        assert!(CliError::UnknownExample("q".to_owned()).source().is_none());
    }

    #[test]
    fn single_summary_does_not_animate() {
        // Only animation draws '@'; its absence proves the summary branch ran.
        let out = run_to_string(&["--example", "open", "--summary"]);
        assert!(!out.contains('@'), "summary mode must not animate");
        assert!(out.contains("expanded:"));
    }

    #[test]
    fn compare_prints_a_full_summary_per_algorithm() {
        let out = run_to_string(&["--example", "rooms", "--compare"]);
        assert_eq!(out.matches("expanded:").count(), 2, "one summary each");
        assert_eq!(out.matches("path cost:").count(), 2);
        let astar = out.find("== A* ==").expect("A* section");
        let greedy = out.find("== greedy ==").expect("greedy section");
        assert!(astar < greedy, "A* should precede greedy");
    }

    #[test]
    fn default_source_is_the_open_example() {
        let default = without_elapsed(run_to_string(&["--summary"]));
        let explicit = without_elapsed(run_to_string(&["--example", "open", "--summary"]));
        assert_eq!(default, explicit, "default source should be example 'open'");
    }

    #[test]
    fn random_same_seed_is_reproducible() {
        let args = &[
            "--random",
            "--width",
            "10",
            "--height",
            "8",
            "--seed",
            "7",
            "--summary",
        ];
        let first = without_elapsed(run_to_string(args));
        let second = without_elapsed(run_to_string(args));
        assert_eq!(first, second, "same seed must reproduce the same run");
    }

    #[test]
    fn no_color_output_has_no_escapes() {
        let out = run_to_string(&["--example", "open", "--summary", "--no-color"]);
        assert!(!out.contains('\x1b'), "no ANSI escapes with --no-color");
    }

    #[test]
    fn eight_connectivity_is_selectable() {
        let out = run_to_string(&[
            "--example",
            "open",
            "--summary",
            "--connectivity",
            "eight",
            "--heuristic",
            "chebyshev",
        ]);
        assert!(out.contains("path cost:"));
    }

    #[test]
    fn algo_arg_to_core_maps_each_variant() {
        assert_eq!(AlgoArg::Astar.to_core(), Algorithm::AStar);
        assert_eq!(AlgoArg::Greedy.to_core(), Algorithm::Greedy);
        assert_eq!(AlgoArg::Dijkstra.to_core(), Algorithm::Dijkstra);
    }

    #[test]
    fn heuristic_arg_to_core_maps_each_variant() {
        assert_eq!(HeuristicArg::Manhattan.to_core(), HeuristicKind::Manhattan);
        assert_eq!(HeuristicArg::Euclidean.to_core(), HeuristicKind::Euclidean);
        assert_eq!(HeuristicArg::Chebyshev.to_core(), HeuristicKind::Chebyshev);
        assert_eq!(HeuristicArg::Zero.to_core(), HeuristicKind::Zero);
    }

    #[test]
    fn conn_arg_to_core_maps_each_variant() {
        assert_eq!(ConnArg::Four.to_core(), Connectivity::Four);
        assert_eq!(ConnArg::Eight.to_core(), Connectivity::Eight);
    }
}
