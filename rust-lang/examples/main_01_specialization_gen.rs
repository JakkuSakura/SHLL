fn inc(i: i64) -> i64 {
    i + 1i64 + 2i64 + 3i64
}
fn double(i: i64) -> i64 {
    i * 2i64
}
fn print(i: i64) {
    println!("{}", i)
}
fn main() {
    print(1i64 + 1i64 + 2i64 + 3i64);
}
pub fn inc_0() -> i64 {
    1i64 + 1i64 + 2i64 + 3i64
}

// stdout: 7i64
// result: ()
