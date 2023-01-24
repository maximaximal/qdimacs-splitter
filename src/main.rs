use clap::Parser;
use file_matcher::FilesNamed;
use std::fs;
use std::path::PathBuf;

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

fn main() {
    let args = Args::parse();

    if args.split.is_some() {
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
