set shell := ["bash", "-lc"]

default:
    @just --list

devx-min:
    scripts/check_devx_min.sh

check-fp example="examples/01_const_eval_basics.fp":
    scripts/check_fp.sh {{example}}

run-examples:
    scripts/run_examples_exec.sh
    scripts/run_examples_bytecode.sh

bench:
    scripts/bench_eight_queens.sh
