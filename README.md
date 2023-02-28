# QDIMACS Splitter

A small splitting utility to ingest `.qdimacs` files and print out
many split formula files with applied assumptions. Splits from the
beginning by default and sets universally quantified variables to
existentially quantified variables if required. 

## Rationale

This produces $2^d$ files, with $d$ being the splitting depth. It
splits along the quantifier prefix. This helps developing new
splitting techniques and to compute the theoretical maximum speedup
that could be gathered by splitting.

## Building and Using

``` bash
cargo build -r
./target/release/qdimacs_splitter
```

Generate merged solution as CNF. Basically a CNF that just maps solutions.
