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
    /// Create a new world project (alias for `world new`)
    New {
        /// Name of the project
        name: String,
        /// Create a 2D world
        #[arg(long = "2d")]
        two_d: bool,
        /// Create a 3D world (default)
        #[arg(long = "3d")]
        three_d: bool,
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
    /// Validate a world project (alias for `world check`)
    Check {
        /// Path to the world project directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Print version information
    Version,
    /// World management commands
    World {
        #[command(subcommand)]
        subcommand: WorldCommands,
    },
}

#[derive(Subcommand)]
enum WorldCommands {
    /// Create a new world project
    New {
        /// Name of the project
        name: String,
        /// Create a 2D world
        #[arg(long = "2d")]
        two_d: bool,
        /// Create a 3D world (default)
        #[arg(long = "3d")]
        three_d: bool,
    },
    /// Validate a world project
    Check {
        /// Path to the world project directory
        #[arg(default_value = ".")]
        path: String,
    },
    /// Publish a new version of the world
    Publish {
        /// Path to the world project directory
        #[arg(default_value = ".")]
        path: String,
        /// Bump major version
        #[arg(long)]
        major: bool,
        /// Bump minor version
        #[arg(long)]
        minor: bool,
        /// Bump patch version (default)
        #[arg(long)]
        patch: bool,
        /// Changelog message
        #[arg(long, default_value = "")]
        changelog: String,
    },
    /// List version history
    Versions {
        /// Path to the world project directory
        #[arg(default_value = ".")]
        path: String,
    },
}

fn resolve_dimension(two_d: bool, three_d: bool) -> Result<&'static str, String> {
    match (two_d, three_d) {
        (true, true) => Err("cannot specify both --2d and --3d".to_string()),
        (true, false) => Ok("2D"),
        _ => Ok("3D"), // default is 3D
    }
}

fn resolve_bump_level(major: bool, minor: bool, patch: bool) -> Result<commands::world::BumpLevel, String> {
    let count = [major, minor, patch].iter().filter(|&&b| b).count();
    if count > 1 {
        return Err("specify at most one of --major, --minor, --patch".to_string());
    }
    if major {
        Ok(commands::world::BumpLevel::Major)
    } else if minor {
        Ok(commands::world::BumpLevel::Minor)
    } else {
        Ok(commands::world::BumpLevel::Patch)
    }
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
        Commands::New {
            name,
            two_d,
            three_d,
        } => resolve_dimension(two_d, three_d)
            .and_then(|dim| commands::new::create_project(&name, dim)),
        Commands::Serve { path, port } => commands::serve::serve_project(&path, Some(port)),
        Commands::Check { path } => commands::check::check_project(&path),
        Commands::World { subcommand } => match subcommand {
            WorldCommands::New {
                name,
                two_d,
                three_d,
            } => resolve_dimension(two_d, three_d)
                .and_then(|dim| commands::new::create_project(&name, dim)),
            WorldCommands::Check { path } => commands::check::check_project(&path),
            WorldCommands::Publish {
                path,
                major,
                minor,
                patch,
                changelog,
            } => resolve_bump_level(major, minor, patch)
                .and_then(|level| commands::world::publish(&path, level, &changelog)),
            WorldCommands::Versions { path } => commands::world::versions(&path),
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
