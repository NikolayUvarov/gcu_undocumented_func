//! `improve`: MIND Hub's client for any computer (100003-APP-0150).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut out = std::io::stdout();
    let mut env = improve::Env { dir: improve::default_dir(), input: &mut input, out: &mut out };
    std::process::exit(improve::run(&args, &mut env));
}
