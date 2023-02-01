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

fn assume_prefix_vars(f: &Formula, filename: &str, v: u32, depth: u32) {
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

fn process_formula_splits(formula: &Formula) {
    let depth: u32 = formula.splits.iter().map(|x| x.vars.len() as u32).sum();
    println!("Depth: {}", depth);
}

fn main() {
    let args = Args::parse();

    if args.split.is_some() {
        let filename = args.split.unwrap();
        let formula_str = fs::read_to_string(&filename).unwrap();
        let formula = parse_qdimacs(&formula_str).unwrap();
        if formula.splits.len() > 0 {
            process_formula_splits(&formula);
        } else {
            let base: u32 = 2;
            let depth = std::cmp::min(args.depth, formula.prefix.len() as u32);
            for i in 0..(base.pow(depth)) {
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

        for f in files.iter() {
            println!("File: {}", f.as_path().display());
        }
    } else {
        println!("!! Require either --split or --merge !!");
    }
}
