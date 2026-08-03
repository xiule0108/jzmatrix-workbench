use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "jzmatrix",
    version,
    about = "JZMatrix Workbench offline-first CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check the local product runtime without contacting a network.
    Doctor {
        /// Emit exactly one versioned JSON response on stdout.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        let mut command = Cli::command();
        let _ = command.print_help();
        eprintln!();
        return ExitCode::from(3);
    };

    match command {
        Commands::Doctor { json } => {
            let response = match matrix_core::default_app_data_dir() {
                Some(path) => matrix_core::run_doctor(&path),
                None => {
                    matrix_core::blocked_response("app_data_unavailable", "无法解析应用数据目录")
                }
            };
            if json {
                match serde_json::to_string(&response) {
                    Ok(serialized) => println!("{serialized}"),
                    Err(_) => return ExitCode::from(4),
                }
            } else {
                println!("jzmatrix doctor: {:?}", &response.status);
            }
            ExitCode::from(response.exit_code() as u8)
        }
    }
}
