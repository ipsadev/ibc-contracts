use anyhow::Result;
use sp1_stellar_prover::Fixture;

use crate::{archive::Archive, stellar_api::StellarApi};

pub enum Evidence {
    Archive(Archive),
    Api(StellarApi),
}

impl Evidence {
    pub fn network_label(&self) -> &'static str {
        match self {
            Self::Archive(archive) => archive.network_label(),
            Self::Api(api) => api.network_label(),
        }
    }

    pub async fn current_ledger(&self) -> Result<u32> {
        match self {
            Self::Archive(archive) => archive.current_ledger().await,
            Self::Api(api) => api.current_ledger().await,
        }
    }

    pub async fn wait_for(&self, ledger: u32) -> Result<()> {
        match self {
            Self::Archive(archive) => archive.wait_for(ledger).await,
            Self::Api(api) => api.wait_for(ledger).await,
        }
    }

    pub async fn fixture(&self, ledger: u32) -> Result<Fixture> {
        match self {
            Self::Archive(archive) => archive.fixture(ledger).await,
            Self::Api(api) => api.fixture(ledger).await,
        }
    }
}
