use clap::{arg, Arg, Command};
use log::error;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    println!("Hello, world!");
    Ok(())
}
