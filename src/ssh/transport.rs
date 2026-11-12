use std::{env, io, process::Command};

mod environment;

fn run() -> io::Result<i32> {
    let mut args = env::args_os().skip(1);
    let program = args
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing transport program"))?;
    let mut command = Command::new(program);
    command.args(args);
    environment::configure(&mut command);
    command.status().map(|status| status.code().unwrap_or(1))
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("ssh: cannot run transport: {error}");
            std::process::exit(1);
        }
    }
}
