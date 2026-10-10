use std::path::PathBuf;

use clap::Parser;
use proof_api::{
    cli::{Commands, ProofApiCli},
    observability::init_observability,
};
use proof_api_core::{builder::ProofApiBuilder, config::ProofApiConfig};
use proof_api_eth_to_stellar::EthToStellarProofApiModule;
use proof_api_forward::ForwardProofApiModule;
use proof_api_stellar_to_eth::StellarToEthProofApiModule;

const INLINE_CONFIG_VARIABLE: &str = "PROOF_API_CONFIG_JSON";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = ProofApiCli::parse();

    match cli.command {
        Commands::Start(arguments) => {
            let configuration_bytes = match std::env::var(INLINE_CONFIG_VARIABLE) {
                Ok(inline) if !inline.trim().is_empty() => inline.into_bytes(),
                _ => std::fs::read(PathBuf::from(arguments.config))?,
            };
            let configuration: ProofApiConfig = serde_json::from_slice(&configuration_bytes)?;

            let _guard = init_observability(&configuration.observability)?;

            let mut builder = ProofApiBuilder::default();

            builder.add_module(StellarToEthProofApiModule);
            builder.add_module(EthToStellarProofApiModule);
            builder.add_module(ForwardProofApiModule);

            tracing::info!(
                address = %configuration.server.address,
                port = configuration.server.port,
                "starting the Stellar proof API"
            );

            builder.start(configuration).await?;

            Ok(())
        }
    }
}
