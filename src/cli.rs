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
    /// Render one or many templates to an output path.
    Build {
        /// Template source: single file, directory (all .tera), or glob pattern (e.g. src/*.tera).
        src: PathBuf,
        /// Output path: for single file, file or directory; for patterns, directory or filename prefix.
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
