use std::env;

mod build;
mod constants;
mod test;
mod utils;

fn main() {
    let arg = env::args().nth(1);
    match arg.as_ref().map(|x| x.as_str()) {
        Some("build") => build::build(),
        Some("test") => test::test(),
        None => eprintln!("no task specified\navailable tasks: build, test"),
        _ => eprintln!("specified task does not exist\navailable tasks: build, test"),
    }
}
