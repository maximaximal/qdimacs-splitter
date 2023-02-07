use bitvec::prelude::*;
use clap::Parser;
use file_matcher::FilesNamed;
use std::fs;
use std::path::{Path, PathBuf};

use qdimacs_splitter::{extract_results_from_files, parse_qdimacs, write_qdimacs, Formula};

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
    /// Depth to split into
    #[arg(short, long, default_value_t = 4)]
    depth: u32,
}

fn get_current_working_dir() -> std::io::Result<PathBuf> {
    std::env::current_dir()
}

fn prefix_name_combiner(filename: &str, name_prefix: Vec<u8>) -> PathBuf {
    let origpath = Path::new(filename);
    let orig_filename = origpath.file_name().unwrap();
    let changed_filename =
        String::from_utf8(name_prefix).unwrap() + ":" + orig_filename.to_str().unwrap();
    let path = Path::new(&changed_filename);
    let mut b = PathBuf::new();
    b.push(path);
    b
}

fn assume_prefix_vars(f: &Formula, filename: &str, v: u64, depth: u64) {
    let mut assumed_f: Formula = Clone::clone(f);

    let mut name_prefix: Vec<u8> = (0..depth).map(|_| 'f' as u8).collect();

    let bv = v.view_bits::<Lsb0>();
    for i in 0..depth as usize {
        let bit = bv[i];
        if assumed_f.prefix[i] > 0 {
            assumed_f.prefix[i] = -assumed_f.prefix[i];
        }
        let abs_lit = assumed_f.prefix[i].abs();
        let lit = if bit { abs_lit } else { -abs_lit };
        if bit {
            name_prefix[i] = 't' as u8;
        }
        assumed_f.matrix.push(vec![lit]);
        assumed_f.nr_of_clauses += 1;
    }

    let out_path = prefix_name_combiner(filename, name_prefix);
    write_qdimacs(&out_path, &assumed_f).unwrap();
}

fn process_formula_splits(formula: &Formula, depth: u32, filename: &str) {
    let splits = formula.produce_splits(depth);

    for i in 0..splits.len() {
        let mut assumed_f: Formula = Clone::clone(formula);
        for v in &splits[i] {
            assumed_f.matrix.push(vec![*v]);
            assumed_f.nr_of_clauses += 1;
        }
        let path = Path::new(filename);
        let out_path_string = i.to_string() + ":" + path.file_name().unwrap().to_str().unwrap();
        let out_path = Path::new(&out_path_string);
        write_qdimacs(out_path, &assumed_f).unwrap();
    }
}

fn main() {
    let args = Args::parse();

    if args.split.is_some() {
        let filename = args.split.unwrap();
        let formula_str = fs::read_to_string(&filename).unwrap();
        let formula = parse_qdimacs(&formula_str).unwrap();
        process_formula_splits(&formula, args.depth, &filename);
    } else if args.orig.is_some() && args.name.is_some() {
        let cwd_buf = get_current_working_dir().unwrap();
        let cwd = cwd_buf.as_path();
        let orig = args.orig.unwrap();
        let name = args.name.unwrap();

        println!("Orig: {:?}, Working Dir: {:?}", orig, cwd);

        let orig_path = Path::new(&orig);

        let results = extract_results_from_files(&orig_path, &name);
        println!("Results: {:?}", results);
    } else {
        println!("!! Require either --split or (--orig and name) !!");
    }
}
