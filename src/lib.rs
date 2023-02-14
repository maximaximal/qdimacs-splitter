extern crate pest;
use bitvec::prelude::*;
#[macro_use]
extern crate pest_derive;
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

use pest::Parser;
use regex::Regex;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;

#[derive(Parser)]
#[grammar = "qdimacs.pest"]
struct QDIMACSParser;

#[derive(Debug, Clone, Copy, PartialEq, strum_macros::Display)]
pub enum SolverReturnCode {
    Sat,
    Unsat,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct SolverResult {
    pub wall_seconds: f64,
    pub result: SolverReturnCode,
}

#[derive(Debug, Clone)]
pub enum IntegerSplitKind {
    LessThan,
    GreaterThan,
    Equals,
}

#[derive(Debug, Clone)]
pub struct IntegerSplitConstraint {
    pub kind: IntegerSplitKind,
    pub target: Vec<Vec<i32>>,
}

fn extract_result_from_file(path: &Path) -> SolverResult {
    lazy_static! {
        static ref EXIT_CODE: Regex =
            Regex::new("Command exited with non-zero status (\\d+)").unwrap();
        static ref WALL_TIME: Regex =
            Regex::new("^\\[runlim\\] real:\\s*(\\d+(?:\\.\\d+))").unwrap();
    }

    let mut wall_seconds: f64 = 0.0;
    let mut result: SolverReturnCode = SolverReturnCode::Timeout;

    let f = File::open(path).unwrap();
    let f = BufReader::new(f);

    for line in f.lines() {
        let line = line.unwrap();
        let exit_code = EXIT_CODE.captures(&line).and_then(|c| {
            c.get(1).and_then(|exit_code| match exit_code.as_str() {
                "10" => Some(SolverReturnCode::Sat),
                "20" => Some(SolverReturnCode::Unsat),
                _ => Some(SolverReturnCode::Timeout),
            })
        });
        let wall_time = WALL_TIME.captures(&line).and_then(|c| {
            c.get(1)
                .and_then(|wall_time| Some(wall_time.as_str().parse::<f64>().unwrap()))
        });
        if exit_code.is_some() {
            result = exit_code.unwrap()
        }
        if wall_time.is_some() {
            wall_seconds = wall_time.unwrap()
        }
    }

    SolverResult {
        wall_seconds,
        result,
    }
}

pub fn extract_results_from_files(
    orig_file: &Path,
    name: &str,
    depth: u32,
    cwd: &Path,
) -> (Formula, Vec<SolverResult>) {
    let formula_str = fs::read_to_string(&orig_file).unwrap();
    let formula = parse_qdimacs(&formula_str).unwrap();
    let splits = formula.produce_splits(depth);
    (
        formula,
        (0..splits.len())
            .map(|n| {
                // Follows the Simsala convention.
                let mut filename: String = String::new();
                filename.push_str(name);
                filename.push_str("-");
                filename.push_str(&n.to_string());
                filename.push_str(":");
                filename.push_str(orig_file.file_name().unwrap().to_str().unwrap());
                filename.push_str(".log");
                let mut p = PathBuf::new();
                p.push(cwd);
                p.push(filename);
                extract_result_from_file(&p.as_path())
            })
            .collect(),
    )
}

fn to_u64(slice: &[i32]) -> u64 {
    slice
        .iter()
        .map(|x| if *x > 0 { 1 } else { 0 })
        .fold(0, |acc, b| acc * 2 + b as u64)
}

impl IntegerSplitConstraint {
    pub fn satisfied(&self, bits: &[i32], num: u64) -> bool {
        match self.kind {
            IntegerSplitKind::LessThan => num < self.target[0][0] as u64,
            IntegerSplitKind::GreaterThan => num > self.target[0][0] as u64,
            IntegerSplitKind::Equals => self.target.iter().any(|tgt| {
                std::iter::zip(bits, tgt)
                    .map(|(v, b)| (*b == 1 && *v > 0) || (*b == 0 && *v < 0))
                    .all(|x| x)
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntegerSplit {
    pub vars: Vec<i32>,
    pub constraints: Vec<IntegerSplitConstraint>,
}

impl IntegerSplit {
    pub fn satisfied_with_num(&self, v: &[i32], num: u64) -> bool {
        self.constraints.iter().any(|x| x.satisfied(v, num))
    }

    pub fn satisfied(&self, v: &[i32]) -> bool {
        assert!(v.len() < 64);
        let num = to_u64(v);
        self.satisfied_with_num(v, num)
    }

    pub fn nr_of_splits(&self) -> usize {
        // Just generate all numbers from 0 to 2^n and check all of
        // them, afterwards count how many passed.
        let base: u64 = 2;
        let n: u64 = base.pow(self.vars.len() as u32);
        (0..n)
            .map(|i| {
                let v: Vec<i32> = n
                    .view_bits::<Lsb0>()
                    .iter()
                    .map(|b| if *b { 1 as i32 } else { 0 as i32 })
                    .collect();
                self.satisfied_with_num(&v, i)
            })
            .filter(|x| *x)
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct Formula {
    pub splits: Vec<IntegerSplit>,
    pub prefix: Vec<i32>,
    pub matrix: Vec<Vec<i32>>,
    pub nr_of_variables: i32,
    pub nr_of_clauses: i32,
}

#[derive(Debug, Clone)]
pub struct FormulaVars<'a> {
    formula: &'a Formula,
    splits_depth: u64,
    current_idx: u64,
    max_idx: u64,
}

impl<'a> Iterator for FormulaVars<'a> {
    type Item = Vec<i32>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_idx + 1 > self.max_idx {
            None
        } else {
            let idx = self.current_idx;
            self.current_idx += 1;
            let bv = idx.view_bits::<Lsb0>();
            let vars: Vec<i32> = self.formula.splits[0..self.splits_depth as usize]
                .iter()
                .map(|x| x.vars.clone().into_iter())
                .flatten()
                .collect();
            let current = std::iter::zip(bv, vars.into_iter().rev())
                .map(|(b, v)| if *b { v } else { (v as i32) * -1 as i32 })
                .rev()
                .collect();
            Some(current)
        }
    }
}

impl Formula {
    pub fn embedded_splits_max_depth(&self) -> usize {
        self.splits.iter().map(|x| x.vars.len()).sum()
    }
    pub fn embedded_splits_round_fitting(&self, mut d: i64) -> (u64, u64) {
        let mut computed_depth: u64 = 0;
        let mut split_count = 0;
        for s in self.splits.iter() {
            d -= s.vars.len() as i64;
            if d >= 0 {
                computed_depth += s.vars.len() as u64;
                split_count += 1;
            } else {
                return (computed_depth, split_count);
            }
        }
        (computed_depth, split_count)
    }
    fn iterate_possible_vars(&self, bit_depth: u64, splits_depth: u64) -> FormulaVars {
        let base: u64 = 2;
        FormulaVars {
            formula: &self,
            splits_depth,
            current_idx: 0,
            max_idx: base.pow(bit_depth as u32),
        }
    }
    fn produce_splits_from_embedded(&self, bit_depth: u64, splits_depth: u64) -> Vec<Vec<i32>> {
        assert!(self.splits.len() > 0);
        let split_var_lengths: Vec<usize> = self.splits[0..splits_depth as usize]
            .iter()
            .map(|split| split.vars.len())
            .collect();
        let split_ranges: Vec<(usize, usize)> = (0..splits_depth as usize)
            .map(|i| {
                let p = split_var_lengths[0..i].iter().sum();
                (p, p + split_var_lengths[i])
            })
            .collect();
        let splits = &self.splits[0..splits_depth as usize];

        let vars = self
            .iterate_possible_vars(bit_depth, splits_depth)
            .filter(|x| {
                std::iter::zip(splits, &split_ranges)
                    .all(|(split, (begin, end))| split.satisfied(&x[*begin..*end]))
            })
            .collect();
        vars
    }
    fn produce_splits_from_prefix_expansion(&self, bit_depth: u64) -> Vec<Vec<i32>> {
        let base: u64 = 2;
        let depth: u64 = std::cmp::min(bit_depth as u64, self.prefix.len() as u64);

        (0..(base.pow(depth as u32)))
            .map(|v| {
                let bv = v.view_bits::<Lsb0>();
                (0..depth as usize)
                    .map(|i| {
                        let bit = bv[i];
                        let abs_lit: i32 = self.prefix[i].abs();
                        if bit {
                            abs_lit
                        } else {
                            -abs_lit
                        }
                    })
                    .collect()
            })
            .collect()
    }
    pub fn produce_splits(&self, depth: u32) -> Vec<Vec<i32>> {
        if self.splits.len() > 0 {
            let (rounded_depth, split_count) = self.embedded_splits_round_fitting(depth as i64);
            self.produce_splits_from_embedded(rounded_depth, split_count)
        } else {
            let depth: u64 = std::cmp::min(depth as u64, self.prefix.len() as u64);
            self.produce_splits_from_prefix_expansion(depth)
        }
    }
}

fn sign(n: i32) -> i32 {
    if n > 0 {
        1
    } else if n < 0 {
        -1
    } else {
        panic!("Don't call sign with 0!");
    }
}

pub fn write_qdimacs(tgt: &Path, formula: &Formula) -> std::io::Result<()> {
    let mut file = BufWriter::new(File::create(tgt).expect("File could not be created!"));
    write!(
        file,
        "p cnf {} {}\n",
        formula.nr_of_variables, formula.nr_of_clauses
    )?;

    let mut last_q: i32 = 0;
    for q_ in formula.prefix.iter() {
        let q = *q_;
        if q < 0 && last_q >= 0 {
            if last_q != 0 {
                write!(file, " 0\n")?;
            }
            write!(file, "e {}", -q)?;
        } else if q > 0 && last_q <= 0 {
            if last_q != 0 {
                write!(file, " 0\n")?;
            }
            write!(file, "a {}", q)?;
        } else {
            if q < 0 {
                write!(file, " {}", -q)?;
            } else {
                write!(file, " {}", q)?;
            }
        }
        last_q = q;
    }

    if last_q != 0 {
        write!(file, " 0\n")?;
    }

    for clause in formula.matrix.iter() {
        let mut space_separated = String::new();

        for l in clause.iter() {
            space_separated.push_str(&l.to_string());
            space_separated.push_str(" ");
        }

        write!(file, "{}0\n", space_separated)?;
    }
    Ok(())
}

fn var_constraint_to_nr_of_bits(v: i32) -> i32 {
    let vf: f64 = v as f64;
    vf.log2().ceil() as i32
}

pub fn parse_qdimacs(qdimacs: &str) -> Result<Formula, String> {
    let file = QDIMACSParser::parse(Rule::file, qdimacs)
        .expect("parser error")
        .next()
        .unwrap();

    let mut nr_of_variables: i32 = 0;
    let mut nr_of_clauses: i32 = 0;
    let mut prefix: Vec<i32> = vec![];
    let mut matrix: Vec<Vec<i32>> = vec![];
    let mut splits: Vec<IntegerSplit> = vec![];

    for line in file.into_inner() {
        match line.as_rule() {
            Rule::problem_line => {
                let mut inner_rules = line.into_inner();
                let nr_of_variables_inner = inner_rules.next().unwrap();
                nr_of_variables = nr_of_variables_inner.as_str().parse::<i32>().unwrap();
                nr_of_clauses = inner_rules.next().unwrap().as_str().parse::<i32>().unwrap();
            }
            Rule::int_split_line => {
                let mut constraints: Vec<IntegerSplitConstraint> = vec![];

                let mut inner_rules = line.into_inner();
                let mut vars_or_cmp = inner_rules.next().unwrap();
                let mut vars_or_cmp_rule = vars_or_cmp.as_rule();
                let mut vars: Vec<i32> = vec![];
                let mut nr_of_bits = 0;
                let mut already_have_next: bool;
                while vars_or_cmp_rule == Rule::pnum {
                    let v = vars_or_cmp.as_str().parse::<i32>().unwrap();
                    vars.push(v);
                    vars_or_cmp = inner_rules.next().unwrap();
                    vars_or_cmp_rule = vars_or_cmp.as_rule();
                }

                loop {
                    already_have_next = false;
                    let mut target: Vec<Vec<i32>> = vec![];
                    let kind: IntegerSplitKind = match vars_or_cmp.as_str() {
                        "<" => IntegerSplitKind::LessThan,
                        ">" => IntegerSplitKind::GreaterThan,
                        "=" => IntegerSplitKind::Equals,
                        p => panic!("Unknown pattern: {}", p),
                    };

                    if matches!(kind, IntegerSplitKind::Equals) {
                        let mut next = inner_rules.next();
                        while next.is_some() {
                            let next_ = next.unwrap();
                            if next_.as_rule() == Rule::onezero {
                                target.push(
                                    next_
                                        .as_str()
                                        .chars()
                                        .map(|x| {
                                            if x == '1' {
                                                1
                                            } else if x == '0' {
                                                0
                                            } else {
                                                panic!(
                                            "Integer Split with = must only have 1 and 0 as match!"
                                        );
                                            }
                                        })
                                        .collect(),
                                );
                                if nr_of_bits == 0 {
                                    nr_of_bits = target[0].len();
                                } else {
                                    if target[target.len() - 1].len() != nr_of_bits {
                                        panic!("Number of assigned bits in equals must always be the same!");
                                    }
                                }
                                next = inner_rules.next();
                            } else {
                                already_have_next = true;
                                break;
                            }
                        }
                    } else {
                        let next = inner_rules.next();
                        assert!(next.is_some());
                        let next_ = next.unwrap();
                        target.push(vec![next_.as_str().parse::<i32>().unwrap()]);
                    }

                    let constraint = IntegerSplitConstraint { kind, target };
                    constraints.push(constraint);

                    if !already_have_next {
                        let next = inner_rules.next();
                        if next.is_some() {
                            vars_or_cmp = next.unwrap();
                        } else {
                            break;
                        }
                    }
                }
                splits.push(IntegerSplit { vars, constraints });
            }
            Rule::quant_set => {
                let mut inner_rules = line.into_inner();
                let quantifier = inner_rules.next().unwrap().as_str();
                let neg = quantifier.eq("e");
                for var in inner_rules {
                    let var_num = var.as_str().parse::<i32>().unwrap();
                    let quantified_var = if neg { var_num * -1 } else { var_num };
                    prefix.push(quantified_var);
                }
            }
            Rule::clause => {
                let mut clause: Vec<i32> = vec![];
                for var in line.into_inner() {
                    let var_num = var.as_str().parse::<i32>().unwrap();
                    clause.push(var_num);
                }
                matrix.push(clause);
            }
            _ => (),
        }
    }

    // Fixup integer splits without assigned variables based on their properties.
    let mut prefix_start = 0;
    for s in splits.iter_mut() {
        if s.constraints.len() < 1 {
            panic!("Require some constraints for int splits!");
        }
        if s.vars.len() == 0 {
            let nr_of_bits = match s.constraints[0].kind {
                IntegerSplitKind::LessThan | IntegerSplitKind::GreaterThan => {
                    var_constraint_to_nr_of_bits(s.constraints[0].target[0][0])
                }
                IntegerSplitKind::Equals => s.constraints[0].target[0].len() as i32,
            };
            s.vars = prefix[(prefix_start as usize)..((prefix_start + nr_of_bits) as usize)]
                .iter()
                .map(|x| x.abs())
                .collect();
            prefix_start += nr_of_bits;
        } else {
            prefix_start += s.vars.len() as i32;
        }
    }

    if splits.len() == 0 && prefix.len() > 0 {
        // Fill integer splits with default splitting, i.e. one
        // variable in order of prefix < 2. Every QBF thus becomes
        // splittable using just this technique!
        let n = std::cmp::min(prefix.len(), 64);
        splits = prefix[0..n]
            .into_iter()
            .map(|p| IntegerSplit {
                vars: vec![p.abs()],
                constraints: vec![IntegerSplitConstraint {
                    kind: IntegerSplitKind::LessThan,
                    target: vec![vec![2]],
                }],
            })
            .collect()
    }

    // Consistency Check with Quantifier Blocks
    for s in splits.iter() {
        let mut last_q = 0;
        for v in s.vars.iter() {
            let q_pos = prefix.iter().position(|q| q.abs() == *v).unwrap();
            let q = prefix[q_pos];
            if last_q != 0 && sign(last_q) != sign(q) {
                panic!("One constraint over multiple different quantifier types! Covered variables {} and {}", last_q, q);
            }
            last_q = q;
        }
    }

    Ok(Formula {
        splits,
        prefix,
        matrix,
        nr_of_variables,
        nr_of_clauses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nr_to_bits() {
        assert_eq!(var_constraint_to_nr_of_bits(5), 3);
        assert_eq!(var_constraint_to_nr_of_bits(3), 2);
        assert_eq!(var_constraint_to_nr_of_bits(2), 1);
    }

    #[test]
    fn test_sign() {
        assert_eq!(sign(2), 1);
        assert_eq!(sign(-2), -1);
    }
}
