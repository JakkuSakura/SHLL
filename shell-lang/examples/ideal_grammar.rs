include!("defs/mod.rs");
use shell_lang::starter::TryStarter;
use shell_lang::*;

fn main() {
    let src = SourceProcess::spawn("src", 1);
    let adder = AddProcess::spawn("adder");
    let dest = SinkProcess::spawn("dest");

    // declarative macro can't create new idents
    shell!(src | adder.add(5) | dest);
    shell!(src | adder.add(6) | dest);
}
// TODO: use move semantics in shell
// to share use &, to cross thread use &Arc
