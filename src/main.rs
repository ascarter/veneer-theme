mod cli;
mod palette;
mod render;
mod show;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Build { src, dest, palette } => {
            render::build(&palette, &src, dest.as_ref())?;
        }
        Command::Check { palette, template } => {
            render::check_single(&palette, &template)?;
        }
        Command::Show { palette } => {
            show::run(&palette)?;
        }
    }

    Ok(())
}
