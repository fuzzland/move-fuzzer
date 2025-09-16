use anyhow::Result;
use clap::{Parser, Subcommand};
use fuzzer_core::fuzzer::CoreFuzzer;
use fuzzer_core::reporter::ConsoleReporter;
use fuzzer_core::FuzzerConfig;
use sui_fuzzer::SuiAdapter;
use tracing::info;

#[derive(Parser)]
#[command(about = "A unified fuzzer for Move-based blockchains")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Supported blockchain commands
#[derive(Subcommand)]
enum Commands {
    /// Fuzz Sui blockchain functions
    Sui {
        #[arg(short, long, help = "RPC URL for Sui network")]
        rpc_url: String,

        #[arg(short, long, help = "Package ID to fuzz")]
        package: String,

        #[arg(short, long, help = "Module name")]
        module: String,

        #[arg(short, long, help = "Function name")]
        function: String,

        #[arg(short = 't', long, num_args = 0.., help = "Type arguments")]
        type_args: Option<Vec<String>>,

        #[arg(short, long, num_args = 0.., help = "Function arguments")]
        args: Vec<String>,

        #[arg(long, default_value = "1000000", help = "Number of iterations")]
        iterations: u64,

        #[arg(long, default_value = "300", help = "Timeout in seconds")]
        timeout: u64,

        #[arg(long, help = "Sender address (optional)")]
        sender: Option<String>,
    },

    /// Fuzz Aptos blockchain functions
    Aptos,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Sui {
            rpc_url,
            package,
            module,
            function,
            type_args,
            args,
            iterations,
            timeout,
            sender,
        } => {
            info!("Starting Sui fuzzing session");

            // Build configuration
            let config = FuzzerConfig {
                rpc_url,
                package_id: package,
                module_name: module,
                function_name: function,
                type_arguments: type_args.unwrap_or_default(),
                args,
                iterations,
                timeout_seconds: timeout,
                sender,
            };

            // Validate configuration
            config.validate()?;

            let reporter = ConsoleReporter::new();
            reporter.print_message(&format!(
                "🎯 Targeting: {}::{}::{}",
                config.package_id, config.module_name, config.function_name
            ))?;

            reporter.print_fuzzing_start(config.iterations, config.timeout_duration())?;

            // Create SuiAdapter and run the fuzzer
            let adapter = SuiAdapter::new(&config.rpc_url).await?;
            let mut fuzzer = CoreFuzzer::new(adapter, config).await?;

            let result = fuzzer.run().await?;
            reporter.print_fuzzing_result(&result)?;
        }

        Commands::Aptos => {
            let reporter = ConsoleReporter::new();
            reporter.print_message("Aptos fuzzing is not yet implemented.")?;
        }
    }

    Ok(())
}
