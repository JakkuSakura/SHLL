#[macro_export]
macro_rules! pipe {
    (inner $proc: tt) => {
        $proc
    };
    (inner $proc1: tt | $proc2: tt) => {
        crate::Pipe::new($proc1, $proc2)
    };
    (inner $proc1: tt | $proc2: tt $(| $proc: tt)+) => {{
        let p = pipe!(inner $proc1 | $proc2);
        pipe!(inner p $(| $proc)+)
    }};

    ($($proc: tt) | +) => {{
        pipe!(inner $((&$proc)) |+)
    }};
}
#[derive(Debug, Clone)]
pub enum Stderr {
    Abort
}


pub trait Actor {
    type Stdout;
    type Stdin;
    fn process(&self, item: Self::Stdin) -> Result<Self::Stdout, Stderr>;
}
impl<'a, T: Actor> Actor for &'a T {
    type Stdout = T::Stdout;
    type Stdin = T::Stdin;

    fn process(&self, item: Self::Stdin) -> Result<Self::Stdout, Stderr> {
        (**self).process(item)
    }
}

pub trait Source: Actor<Stdin=()> {
    fn start(&self) -> Result<Self::Stdout, Stderr>;
}

impl<T: Actor<Stdin=()>> Source for T {
    fn start(&self) -> Result<Self::Stdout, Stderr> {
        self.process(())
    }
}

pub trait Sink: Actor<Stdout=()> {}

impl<T: Actor<Stdout=()>> Sink for T {}


pub struct Pipe<L: Actor, R: Actor> {
    l: L,
    r: R,
}

impl<L: Actor, R: Actor<Stdin=L::Stdout>> Pipe<L, R> {
    pub fn new(l: L, r: R) -> Self {
        Self {
            l,
            r,
        }
    }
}

impl<L: Actor, R: Actor<Stdin=L::Stdout>> Actor for Pipe<L, R> {
    type Stdout = R::Stdout;
    type Stdin = L::Stdin;
    fn process(&self, item: Self::Stdin) -> Result<Self::Stdout, Stderr> {
        let out = self.l.process(item)?;
        self.r.process(out)
    }
}