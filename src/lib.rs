extern crate pest;
#[macro_use]
extern crate pest_derive;
use std::path::Path;

use pest::Parser;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;

#[derive(Parser)]
#[grammar = "qdimacs.pest"]
struct QDIMACSParser;

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

#[derive(Debug, Clone)]
pub struct IntegerSplit {
    pub vars: Vec<i32>,
    pub constraints: Vec<IntegerSplitConstraint>,
}

#[derive(Debug, Clone)]
pub struct Formula {
    pub splits: Vec<IntegerSplit>,
    pub prefix: Vec<i32>,
    pub matrix: Vec<Vec<i32>>,
    pub nr_of_variables: i32,
    pub nr_of_clauses: i32,
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
