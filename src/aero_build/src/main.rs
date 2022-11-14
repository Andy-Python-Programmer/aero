use std::env;

mod build;
mod constants;
mod test;
mod utils;

fn main() {
    let arg = env::args().nth(1);
    match arg.as_ref().map(|x| x.as_str()) {
        Some("build") => build::build(),
        // Some("test") => test::test(),
        Some("testing") => testing(),
        None => eprintln!("no task specified\navailable tasks: build, test"),
        _ => eprintln!("specified task does not exist\navailable tasks: build, test"),
    }
}

fn testing() {
    use std::process::{Command, Stdio};

    use execute::{Execute, shell};
    
    let mut command = shell("./aero.py");
    
    command.stdout(Stdio::piped());
    
    let output = command.execute_output().unwrap();
    
    println!("{}", String::from_utf8(output.stdout).unwrap()); 
}
