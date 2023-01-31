extern crate pest;
#[macro_use]
extern crate pest_derive;

use pest::Parser;
use std::fs::File;
use std::io::prelude::*;

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
pub struct IntegerSplit {
    kind: IntegerSplitKind,
    vars: Vec<i32>,
    target: Vec<Vec<i32>>,
}

#[derive(Debug, Clone)]
pub struct Formula {
    pub prefix: Vec<i32>,
    pub matrix: Vec<Vec<i32>>,
    pub nr_of_variables: i32,
    pub nr_of_clauses: i32,
}

pub fn write_qdimacs(tgt: &str, formula: &Formula) -> std::io::Result<()> {
    let mut file = File::create(tgt)?;
    write!(
        &mut file,
        "p cnf {} {}\n",
        formula.nr_of_variables, formula.nr_of_clauses
    )?;

    let mut last_q: i32 = 0;
    for q_ in formula.prefix.iter() {
        let q = *q_;
        if q < 0 && last_q >= 0 {
            if last_q != 0 {
                write!(&mut file, " 0\n")?;
            }
            write!(&mut file, "e {}", -q)?;
        } else if q > 0 && last_q <= 0 {
            if last_q != 0 {
                write!(&mut file, " 0\n")?;
            }
            write!(&mut file, "a {}", q)?;
        } else {
            if q < 0 {
                write!(&mut file, " {}", -q)?;
            } else {
                write!(&mut file, " {}", q)?;
            }
        }
        last_q = q;
    }

    if last_q != 0 {
        write!(&mut file, " 0\n")?;
    }

    for clause in formula.matrix.iter() {
        let mut space_separated = String::new();

        for l in clause.iter() {
            space_separated.push_str(&l.to_string());
            space_separated.push_str(" ");
        }

        write!(&mut file, "{}0\n", space_separated)?;
    }
    Ok(())
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
    let mut integer_splits: Vec<IntegerSplit> = vec![];

    for line in file.into_inner() {
        match line.as_rule() {
            Rule::problem_line => {
                let mut inner_rules = line.into_inner();
                let nr_of_variables_inner = inner_rules.next().unwrap();
                nr_of_variables = nr_of_variables_inner.as_str().parse::<i32>().unwrap();
                nr_of_clauses = inner_rules.next().unwrap().as_str().parse::<i32>().unwrap();
            }
            Rule::int_split_line => {
                let mut inner_rules = line.into_inner();
                let mut vars_or_cmp = inner_rules.next().unwrap();
                let mut vars_or_cmp_rule = vars_or_cmp.as_rule();
                let mut vars: Vec<i32> = vec![];
                let mut target: Vec<Vec<i32>> = vec![];
                while vars_or_cmp_rule == Rule::pnum {
                    let v = vars_or_cmp.as_str().parse::<i32>().unwrap();
                    vars.push(v);
                    println!("V: {}", v);
                    vars_or_cmp = inner_rules.next().unwrap();
                    vars_or_cmp_rule = vars_or_cmp.as_rule();
                }

                let kind: IntegerSplitKind = match vars_or_cmp.as_str() {
                    "<" => IntegerSplitKind::LessThan,
                    ">" => IntegerSplitKind::GreaterThan,
                    "=" => IntegerSplitKind::Equals,
                    _ => panic!("Unknown pattern!"),
                };

                if matches!(kind, IntegerSplitKind::Equals) {
                    let mut num = inner_rules.next();
                    while num.is_some() {
                        target.push(
                            num.unwrap()
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
                        num = inner_rules.next();
                    }
                } else {
                    target.push(vec![inner_rules
                        .next()
                        .unwrap()
                        .as_str()
                        .parse::<i32>()
                        .unwrap()]);
                }

                integer_splits.push(IntegerSplit { kind, vars, target });
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

    Ok(Formula {
        prefix,
        matrix,
        nr_of_variables,
        nr_of_clauses,
    })
}
