use clap::Parser;
use indicatif::ProgressBar;
use std::{fs, iter::zip};
use strsim::jaro;

/// CLI input arguments.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// File to process.
    #[arg(short, long)]
    file: std::path::PathBuf,
    /// Similarity threshold in [0, 1]. Only collections of lines that are more similar
    /// than this will be considered.
    #[arg(short, long, default_value_t = 0.9)]
    thres: f64,
}

/// Line range with a beginning (inclusive) and end (inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LineRange {
    /// Start of the range (inclusive).
    start: usize,
    /// End of the range (inclusive).
    end: usize,
}

/// Test how similar two ranges of lines are.
///
/// Returns `true` if the given ranges `range1` and `range2` are more similar than the
/// given threshold `thres`.
fn test_ranges(range1: &LineRange, range2: &LineRange, raw_lines: &[&str], thres: f64) -> bool {
    let n = range1.end - range1.start + 1;
    if (range2.end - range2.start + 1) != n {
        panic!("Invalid ranges {range1:?} and {range2:?}.");
    }
    let lines1 = &raw_lines[range1.start..=range1.end];
    let lines2 = &raw_lines[range2.start..=range2.end];

    let mut sim_sum: f64 = 0.0;
    zip(lines1, lines2).for_each(|(line1, line2)| sim_sum += jaro(line1.trim(), line2.trim()));
    (sim_sum / (n as f64)) > thres
}

/// Line range expansion position.
enum Position {
    /// The line range will be expanded towards the top.
    Start,
    /// The line range will be expanded towards the bottom.
    End,
}

/// Add a row to the given line range `range`, according to `position`.
fn add_row(range: &LineRange, position: &Position) -> LineRange {
    LineRange {
        start: match position {
            Position::Start => range.start - 1,
            Position::End => range.start,
        },
        end: match position {
            Position::Start => range.end,
            Position::End => range.end + 1,
        },
    }
}

/// Grow `range1` and `range2` at the given  `position`.
///
/// Returns `Some(true, out_range1, out_range2)`, if the ranges could be grown
/// successfully, where `out_range1` and `out_range2` correspond to the expanded ranges.
/// Otherwise, returns `Some(false, out_range1, out_range2)`.
/// If the ranges could be grown successfully but this was already recorded previously
/// in `visited`, `None` is returned.
fn grow_at_position(
    range1: LineRange,
    range2: LineRange,
    raw_lines: &[&str],
    thres: f64,
    visited: &mut Vec<(LineRange, LineRange)>,
    position: &Position,
    n_lines: usize,
) -> Option<(bool, LineRange, LineRange)> {
    let condition = match position {
        Position::Start => (range1.start >= 1) && (range2.start >= 1),
        Position::End => (range1.end < (n_lines - 1)) && (range2.end < (n_lines - 1)),
    };
    if condition {
        let trial_range1 = add_row(&range1, position);
        let trial_range2 = add_row(&range2, position);
        if test_ranges(&trial_range1, &trial_range2, raw_lines, thres) {
            if visited.contains(&(trial_range1, trial_range2)) {
                return None;
            }
            visited.push((trial_range1, trial_range2));
            return Some((true, trial_range1, trial_range2));
        }
    }
    Some((false, range1, range2))
}

/// Starting from `range1` and `range2`, assimilate similar surrounding lines until they
/// are no longer similar enough (as measured by `thres`).
///
/// `visited` and `leaves` are modified in-place to record the matching sets of lines,
/// where `leaves` will contain only the subset of `visited` that consists of the
/// largest intersections without any intermediaries.
fn grow_ranges(
    mut range1: LineRange,
    mut range2: LineRange,
    raw_lines: &Vec<&str>,
    thres: f64,
    visited: &mut Vec<(LineRange, LineRange)>,
    leaves: &mut Vec<(LineRange, LineRange)>,
) {
    let n_lines = raw_lines.len();

    if (range1.end.abs_diff(range2.start) == 1) || (range1.start.abs_diff(range2.end) == 1) {
        return;
    }

    loop {
        let mut grew_both = true;
        for position in [Position::Start, Position::End] {
            if let Some((grew, out_range1, out_range2)) = grow_at_position(
                range1, range2, raw_lines, thres, visited, &position, n_lines,
            ) {
                if !grew {
                    grew_both = false;
                }
                range1 = out_range1;
                range2 = out_range2;
            } else {
                return;
            };
        }

        if !grew_both {
            break;
        }
    }
    leaves.push((range1, range2));
}

/// Find similar lines within a given file.
fn main() {
    let args = Args::parse();
    let thres = args.thres;

    let mut visited: Vec<(LineRange, LineRange)> = Vec::new();
    let mut leaves: Vec<(LineRange, LineRange)> = Vec::new();

    let Ok(file_path) = args.file.canonicalize() else {
        panic!("Invalid file '{:?}'", args.file)
    };

    let contents = fs::read_to_string(file_path).expect("Unable to read file.");

    let lines_iter = contents.lines();
    let raw_lines: &Vec<&str> = &lines_iter.clone().collect();

    let n = raw_lines.len() - 1;
    let bar = ProgressBar::new((n * (n + 1) / 2).try_into().unwrap());
    for (i, line1) in lines_iter.clone().enumerate() {
        for (j_i, line2) in lines_iter.clone().skip(i + 1).enumerate() {
            // Correct for the offset applied using `skip`.
            let j = j_i + i + 1;
            // Trim lines to enable comparison.
            let trimmed_lines = [line1, line2].map(|s| s.trim());
            if jaro(trimmed_lines[0], trimmed_lines[1]) > thres {
                grow_ranges(
                    LineRange { start: i, end: i },
                    LineRange { start: j, end: j },
                    raw_lines,
                    thres,
                    &mut visited,
                    &mut leaves,
                );
            }
            bar.inc(1);
        }
    }
    bar.finish();

    leaves.into_iter().for_each(|(lines1, lines2)| {
        if (lines2.end - lines2.start) >= 5 {
            println!("{}", raw_lines[lines1.start..=lines1.end].join("\n"));
            println!("------");
            println!("{}", raw_lines[lines2.start..=lines2.end].join("\n"));
            println!("------");
            println!("------");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn testing_similar_ranges() {
        assert!(test_ranges(
            &LineRange { start: 0, end: 1 },
            &LineRange { start: 2, end: 3 },
            &vec!["abc", "def", "abc", "deg"],
            0.8,
        ));
    }

    #[test]
    fn row_addition() {
        assert_eq!(
            add_row(&LineRange { start: 1, end: 2 }, &Position::Start),
            LineRange { start: 0, end: 2 }
        );
        assert_eq!(
            add_row(&LineRange { start: 1, end: 2 }, &Position::End),
            LineRange { start: 1, end: 3 }
        );
    }

    #[test]
    fn grow_line_ranges() {
        let mut visited: Vec<(LineRange, LineRange)> = Vec::new();

        grow_at_position(
            LineRange { start: 0, end: 0 },
            LineRange { start: 2, end: 2 },
            &vec!["abc", "def", "abc", "deg"],
            0.8,
            &mut visited,
            &Position::End,
            4,
        );
        assert_eq!(
            visited,
            vec![(
                LineRange { start: 0, end: 1 },
                LineRange { start: 2, end: 3 }
            )]
        );

        let mut leaves: Vec<(LineRange, LineRange)> = Vec::new();
        visited = Vec::new();

        grow_ranges(
            LineRange { start: 0, end: 0 },
            LineRange { start: 2, end: 2 },
            &vec!["abc", "def", "abc", "deg"],
            0.8,
            &mut visited,
            &mut leaves,
        );
        assert_eq!(
            visited,
            vec![(
                LineRange { start: 0, end: 1 },
                LineRange { start: 2, end: 3 }
            )]
        );
        assert_eq!(
            leaves,
            vec![(
                LineRange { start: 0, end: 1 },
                LineRange { start: 2, end: 3 }
            )]
        );
    }
}
