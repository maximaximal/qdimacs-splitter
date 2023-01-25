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

    for line in file.into_inner() {
        match line.as_rule() {
            Rule::problem_line => {
                let mut inner_rules = line.into_inner();
                let nr_of_variables_inner = inner_rules.next().unwrap();
                nr_of_variables = nr_of_variables_inner.as_str().parse::<i32>().unwrap();
                nr_of_clauses = inner_rules.next().unwrap().as_str().parse::<i32>().unwrap();
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
