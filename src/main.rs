use clap::Parser;
use nexus_albaai::cli::commands::{
    handle_approve, handle_cancel, handle_deny, handle_fork, handle_resume, handle_run,
    handle_status, Cli, Commands,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => handle_run(args).await,
        Commands::Status(args) => handle_status(args).await,
        Commands::Approve(args) => handle_approve(args).await,
        Commands::Deny(args) => handle_deny(args).await,
        Commands::Resume(args) => handle_resume(args).await,
        Commands::Cancel(args) => handle_cancel(args).await,
        Commands::Fork(args) => handle_fork(args).await,
    }
}
