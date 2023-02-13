use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

use qdimacs_splitter::{
    extract_results_from_files, parse_qdimacs, write_qdimacs, Formula, SolverResult,
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
    #[arg(short, long)]
    name: Option<String>,
    /// Directory to search files to merge or to write files to. Is the current working directory by default.
    #[arg(short, long)]
    working_directory: Option<String>,
    /// Depth to split into. Also required for merging files to see how many files to parse.
    #[arg(short, long, default_value_t = 4)]
    depth: u32,
}

fn get_current_working_dir() -> std::io::Result<PathBuf> {
    std::env::current_dir()
}

fn process_formula_splits(formula: &Formula, depth: u32, filename: &str, working_directory: &Path) {
    let splits = formula.produce_splits(depth);

    for i in 0..splits.len() {
        let mut assumed_f: Formula = Clone::clone(formula);
        for v in &splits[i] {
            assumed_f.matrix.push(vec![*v]);
            assumed_f.nr_of_clauses += 1;
        }
        let path = Path::new(filename);
        let out_path_string = i.to_string() + ":" + path.file_name().unwrap().to_str().unwrap();
        let mut out_path = PathBuf::new();
        out_path.push(working_directory);
        out_path.push(out_path_string);
        write_qdimacs(out_path.as_path(), &assumed_f).unwrap();
    }
}

fn produce_statistics_from_run(formula: &Formula, results: &[SolverResult]) {}

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
        let formula = parse_qdimacs(&formula_str).unwrap();
        process_formula_splits(&formula, args.depth, &filename, working_directory.as_path());
    } else if args.orig.is_some() && args.name.is_some() {
        let cwd = working_directory.as_path();
        let orig = args.orig.unwrap();
        let name = args.name.unwrap();

        let orig_path = Path::new(&orig);
        if !orig_path.exists() {
            println!("!! Original File {} does not exist !!", orig);
        } else {
            let (formula, results) = extract_results_from_files(&orig_path, &name, args.depth, cwd);
            produce_statistics_from_run(&formula, &results);
            println!("Results: {:?}", results);
        }
    } else {
        println!("!! Require either --split or (--orig and name) !!");
    }
}
