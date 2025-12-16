use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Veneer CLI entrypoint.
#[derive(Parser, Debug)]
#[command(name = "veneer", version, about = "Simple theme generator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render a template to an output path.
    Build {
        /// Template file to render (must end with .tera).
        src: PathBuf,
        /// Output file or directory (default: current directory).
        dest: Option<PathBuf>,
        /// Palette TOML file.
        #[arg(long, default_value = "veneer.toml")]
        palette: PathBuf,
    },
    /// Validate palette + template without writing outputs.
    Check {
        /// Palette TOML file.
        #[arg(long, default_value = "veneer.toml")]
        palette: PathBuf,
        /// Template file to render (must end with .tera).
        template: PathBuf,
    },
    /// Show palette values with color swatches.
    Show {
        /// Palette TOML file.
        #[arg(long, default_value = "veneer.toml")]
        palette: PathBuf,
    },
}
