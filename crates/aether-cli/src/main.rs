use std::process;

use clap::{Parser, Subcommand};

use aether_cli::commands;

#[derive(Parser)]
#[command(name = "aether", version, about = "Aether VR Engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a built-in example
    Run {
        /// Name of the example to run
        name: Option<String>,
        /// List available examples
        #[arg(short, long)]
        list: bool,
    },
    /// Create a new world project
    New {
        /// Name of the project
        name: String,
    },
    /// Start a local development server for a world project
    Serve {
        /// Path to the world project directory
        #[arg(default_value = ".")]
        path: String,
        /// Port to listen on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },
    /// Validate a world project
    Check {
        /// Path to the world project directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Print version information
    Version,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Version => {
            commands::version::print_version();
            Ok(())
        }
        Commands::Run { name, list } => {
            if list || name.is_none() {
                commands::run::list_examples();
                Ok(())
            } else {
                commands::run::run_example(name.as_deref().unwrap())
            }
        }
        Commands::New { name } => commands::new::create_project(&name),
        Commands::Serve { path, port } => commands::serve::serve_project(&path, Some(port)),
        Commands::Check { path } => commands::check::check_project(&path),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
