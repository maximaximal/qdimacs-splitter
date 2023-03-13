use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

use qdimacs_splitter::{
    extract_result_from_file, extract_results_from_files, parse_qdimacs, write_qdimacs, Formula,
    IntegerSplit, SolverResult, SolverReturnCode,
};

/// Tool to explore a QBF formula together with a QBF solver to aid
/// during the encoding debugging process.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file to process
    #[arg(short, long)]
    split: Option<String>,
    /// Original input file to merge together. Also requires the splitting depth and name of the run.
    #[arg(short, long)]
    orig: Option<String>,
    /// Name of the run to merge.
    #[arg(short, long, num_args = 1.., value_delimiter = ',')]
    name: Option<Vec<String>>,
    /// Directory to search files to merge or to write files to. Is the current working directory by default.
    #[arg(short, long)]
    working_directory: Option<String>,
    /// Depth to split into. Also required for merging files to see how many files to parse.
    #[arg(short, long, default_value_t = 4)]
    depth: u32,
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
}

fn get_current_working_dir() -> std::io::Result<PathBuf> {
    std::env::current_dir()
}

fn process_formula_splits(
    formula: &Formula,
    depth: u32,
    filename: &str,
    working_directory: &Path,
    verbose: bool,
) {
    let splits = formula.produce_splits(depth);

    for i in 0..splits.len() {
        let mut assumed_f: Formula = Clone::clone(formula);
        for j in 0..splits[i].len() {
            let v = &splits[i][j];
            // Flip forall quantifiers to existential if there is a specific assignment.
            if !assumed_f.prefix.is_empty() && assumed_f.prefix[j] > 0 {
                assumed_f.prefix[j] = -assumed_f.prefix[j];
            }
            assumed_f.matrix.push(vec![*v]);
            assumed_f.nr_of_clauses += 1;
        }
        let path = Path::new(filename);
        let out_path_string = i.to_string() + ":" + path.file_name().unwrap().to_str().unwrap();
        let mut out_path = PathBuf::new();
        out_path.push(working_directory);
        out_path.push(out_path_string);
        if verbose {
            println!(
                "Split with variables {:?} into {:?}",
                splits[i],
                out_path.as_path()
            );
        }
        write_qdimacs(out_path.as_path(), &assumed_f).unwrap();
    }
}

#[derive(Debug)]
struct SolveStatistics {
    pub minimal_execution_time_seconds: f64,
    pub summed_execution_time_seconds: f64,
    pub non_split_execution_time_seconds: f64,
    pub speedup_against_non_split: f64,
    pub required_cores: i32,
    pub result: SolverReturnCode,
    pub naive_split_count: i32,
    pub run_tasks_compared_to_naive: f64,
}

enum Quantifier {
    Forall,
    Exists,
}

// Reduce the result by one layer.
fn reduce_result(
    quant: Quantifier,
    single_layer_width: usize,
    results: Vec<SolverResult>,
) -> Vec<SolverResult> {
    assert!(single_layer_width > 0);
    let result_count = results.len() / single_layer_width;
    (0..result_count)
        .map(|x| {
            let begin = x * single_layer_width;
            let end = (x + 1) * single_layer_width;

            let resit = || (begin..end).map(|x| &results[x]);
            let min_of = |compare_against: SolverReturnCode| -> &SolverResult {
                resit()
                    .filter(|x| x.result == compare_against)
                    .min_by(|a, b| {
                        a.wall_seconds
                            .partial_cmp(&b.wall_seconds)
                            .expect("Tried to compare a NaN")
                    })
                    .unwrap()
            };
            let max_of = |compare_against: SolverReturnCode| -> &SolverResult {
                resit()
                    .filter(|x| x.result == compare_against)
                    .max_by(|a, b| {
                        a.wall_seconds
                            .partial_cmp(&b.wall_seconds)
                            .expect("Tried to compare a NaN")
                    })
                    .unwrap()
            };

            if matches!(quant, Quantifier::Exists) {
                if resit().any(|r| matches!(r.result, SolverReturnCode::Sat)) {
                    let min = min_of(SolverReturnCode::Sat);
                    SolverResult {
                        wall_seconds: min.wall_seconds,
                        result: SolverReturnCode::Sat,
                        name: min.name.to_owned(),
                    }
                } else {
                    if resit().all(|r| matches!(r.result, SolverReturnCode::Unsat)) {
                        let max = max_of(SolverReturnCode::Unsat);
                        SolverResult {
                            wall_seconds: max.wall_seconds,
                            result: SolverReturnCode::Unsat,
                            name: max.name.to_owned(),
                        }
                    } else {
                        SolverResult {
                            wall_seconds: 10000000.0,
                            result: SolverReturnCode::Timeout,
                            name: "(no solver)".to_string(),
                        }
                    }
                }
            } else {
                if resit().all(|r| matches!(r.result, SolverReturnCode::Sat)) {
                    let max = max_of(SolverReturnCode::Sat);
                    SolverResult {
                        wall_seconds: max.wall_seconds,
                        result: SolverReturnCode::Sat,
                        name: max.name.to_owned(),
                    }
                } else {
                    if resit().any(|r| matches!(r.result, SolverReturnCode::Unsat)) {
                        let min = min_of(SolverReturnCode::Unsat);
                        SolverResult {
                            wall_seconds: min.wall_seconds,
                            result: SolverReturnCode::Unsat,
                            name: min.name.to_owned(),
                        }
                    } else {
                        SolverResult {
                            wall_seconds: 10000000.0,
                            result: SolverReturnCode::Timeout,
                            name: "(no solver)".to_string(),
                        }
                    }
                }
            }
        })
        .collect()
}

fn quant_from_prefix(formula: &Formula, pos: usize) -> Quantifier {
    if pos >= formula.prefix.len() {
        // This case is important for merging DIMACS files.
        Quantifier::Exists
    } else if formula.prefix[pos] < 0 {
        Quantifier::Exists
    } else {
        Quantifier::Forall
    }
}

fn produce_statistics_from_run(
    formula: &Formula,
    results: &[SolverResult],
    split_count: u64,
    og_formula_result: Option<SolverResult>,
) -> SolveStatistics {
    let splits: Vec<&IntegerSplit> = formula.splits[0..split_count as usize]
        .into_iter()
        .rev()
        .collect();

    // The splits at this point contain all results in full detail.
    // Each element in the vector maps to some problem instance that
    // was split from the original formula.

    let splits_depth: usize = splits.iter().map(|x| x.vars.len()).sum();

    let base: i32 = 2;
    let naive_split_count = base.pow(splits_depth as u32);

    let required_cores = results.len() as i32;

    let summed_execution_time_seconds: f64 = results.iter().map(|x| x.wall_seconds).sum();

    let mut quanttree_pos = splits_depth - 1;
    let mut solver_results: Vec<SolverResult> = results.to_vec();
    for s in splits.into_iter() {
        let n = s.nr_of_splits();
        solver_results = reduce_result(
            quant_from_prefix(&formula, quanttree_pos),
            n,
            solver_results,
        );
        if quanttree_pos > s.vars.len() {
            quanttree_pos -= s.vars.len();
        }
    }

    assert!(solver_results.len() == 1);

    let minimal_execution_time_seconds: f64 = solver_results[0].wall_seconds;

    let mut non_split_execution_time_seconds: f64 = 10000000.0;
    let mut speedup_against_non_split: f64 = 0.0;

    if let Some(r) = og_formula_result {
        non_split_execution_time_seconds = r.wall_seconds;
        speedup_against_non_split =
            non_split_execution_time_seconds / minimal_execution_time_seconds;
    }

    SolveStatistics {
        minimal_execution_time_seconds,
        summed_execution_time_seconds,
        required_cores,
        result: solver_results[0].result,
        non_split_execution_time_seconds,
        speedup_against_non_split,
        naive_split_count,
        run_tasks_compared_to_naive: required_cores as f64 / naive_split_count as f64,
    }
}

fn main() {
    let args = Args::parse();

    let working_directory: PathBuf = args
        .working_directory
        .and_then(|x| {
            let mut b = PathBuf::new();
            b.push(x);
            Some(b)
        })
        .unwrap_or_else(|| get_current_working_dir().unwrap());

    if args.split.is_some() {
        let filename = args.split.unwrap();
        let formula_str = fs::read_to_string(&filename).unwrap();
        let formula = parse_qdimacs(&formula_str, args.verbose).unwrap();
        process_formula_splits(
            &formula,
            args.depth,
            &filename,
            working_directory.as_path(),
            args.verbose,
        );
    } else if args.orig.is_some() && args.name.is_some() {
        let cwd = working_directory.as_path();
        let orig = args.orig.unwrap();
        let name = args.name.unwrap();

        let orig_path = Path::new(&orig);
        if !orig_path.exists() {
            println!("!! Original File {} does not exist !!", orig);
        } else {
            let (formula, results) = extract_results_from_files(&orig_path, &name, args.depth, cwd);
            let (_rounded_depth, split_count) =
                formula.embedded_splits_round_fitting(args.depth as i64);

            let mut orig_file_result = PathBuf::new();
            orig_file_result.push(cwd);
            orig_file_result.push(
                name[0].to_owned()
                    + "-"
                    + orig_path.file_name().unwrap().to_str().unwrap()
                    + ".log",
            );

            let og_formula_result: Option<SolverResult> = if orig_file_result.exists() {
                Some(extract_result_from_file(
                    orig_file_result.as_path(),
                    &name[0],
                ))
            } else {
                None
            };

            let statistics =
                produce_statistics_from_run(&formula, &results, split_count, og_formula_result);
            println!("Statistics: minimal execution path: {} , summed execution time: {} , required cores: {} , result: {}, naive split count: {} (compared to naive splits: {})",
                     statistics.minimal_execution_time_seconds,
                     statistics.summed_execution_time_seconds,
                     statistics.required_cores,
                     statistics.result,
                     statistics.naive_split_count,
                     statistics.run_tasks_compared_to_naive);
            if orig_file_result.exists() {
                println!(
                    "Original solve time: {} gives speedup of {}",
                    statistics.non_split_execution_time_seconds,
                    statistics.speedup_against_non_split
                );
            } else {
                println!(
                    "No statistics compared to non-split solving, as file {:?} not found.",
                    orig_file_result
                )
            }
        }
    } else {
        println!("!! Require either --split or (--orig and name) !!");
    }
}
