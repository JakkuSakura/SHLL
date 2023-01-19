fn inc(i: i64) -> i64 {
    i + 1i64
}
fn double(i: i64) -> i64 {
    i * 2i64
}
fn print(i: i64) -> () {
    println!("{}", i)
}
fn main() -> () {
    print(inc_0());
}
fn inc_0() -> i64 {
    1i64 + 1i64
}

// stdout: 2i64
// result: Unit
