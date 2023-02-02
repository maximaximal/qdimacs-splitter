use bitvec::prelude::*;
use clap::Parser;
use file_matcher::FilesNamed;
use std::fs;
use std::path::{Path, PathBuf};

use qdimacs_splitter::{parse_qdimacs, write_qdimacs, Formula};

/// Tool to explore a QBF formula together with a QBF solver to aid
/// during the encoding debugging process.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input file to process
    #[arg(short, long)]
    split: Option<String>,
    /// Input file to process. Separate multiple files with spaces or supply wildcards like "*_orig_formula.qdimacs"
    #[arg(short, long, use_value_delimiter = true, value_delimiter = ' ')]
    merge: Option<Vec<String>>,
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

fn process_formula_splits(formula: &Formula, bit_depth: u64, splits_depth: u64, filename: &str) {
    let splits = formula.produce_splits(bit_depth, splits_depth);

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
        if formula.splits.len() > 0 {
            let depth: u64 = std::cmp::min(
                args.depth as u64,
                formula.embedded_splits_max_depth() as u64,
            );
            let (rounded_depth, split_count) = formula.embedded_splits_round_fitting(depth as i64);
            process_formula_splits(&formula, rounded_depth, split_count, &filename);
        } else {
            let base: u64 = 2;
            let depth: u64 = std::cmp::min(args.depth as u64, formula.prefix.len() as u64);
            for i in 0..(base.pow(depth as u32)) {
                assume_prefix_vars(&formula, &filename, i, depth);
            }
        }
    } else if args.merge.is_some() {
        let cwd_buf = get_current_working_dir().unwrap();
        let cwd = cwd_buf.as_path();
        let files: Vec<PathBuf> = args
            .merge
            .unwrap()
            .iter()
            .map(|x| FilesNamed::wildmatch(x).within(cwd).find().unwrap())
            .flatten()
            .collect();

        if files.len() == 1 {

        } else {
            panic!("Merging tftftf files (exhaustive splits) not implemented yet!");
        }
    } else {
        println!("!! Require either --split or --merge !!");
    }
}
