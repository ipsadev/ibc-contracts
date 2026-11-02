// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use ipsa_scp_exporter::{
    api::{export, Options},
    archive::checkpoint_for,
    network::Network,
};
use serde_json::Value;
use sp1_stellar_prover::Fixture;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

pub struct Archive {
    options: Options,
    http: reqwest::Client,
    checkpoint_timeout: Duration,
}

impl Archive {
    pub fn new(
        network: &str,
        archive_urls: Option<Vec<String>>,
        checkpoint_timeout: Duration,
    ) -> Result<Self> {
        let network = Network::parse(network)
            .map_err(|error| anyhow!("unknown Stellar network {network}: {error}"))?;
        let mut options = Options::for_network(network);

        if let Some(urls) = archive_urls.filter(|urls| !urls.is_empty()) {
            options.archives = urls;
            options.archives_pinned = true;
        }

        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("building the history archive http client")?;

        Ok(Self {
            options,
            http,
            checkpoint_timeout,
        })
    }

    pub fn network_label(&self) -> &'static str {
        self.options.network.label()
    }

    pub async fn current_ledger(&self) -> Result<u32> {
        let url = format!(
            "{}/.well-known/stellar-history.json",
            self.options.archives[0].trim_end_matches('/')
        );
        let state: Value = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("reading {url}"))?
            .json()
            .await
            .with_context(|| format!("{url} is not JSON"))?;

        let current = state
            .get("currentLedger")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("{url} carries no currentLedger"))?;

        u32::try_from(current).map_err(|_| anyhow!("currentLedger {current} does not fit a u32"))
    }

    pub async fn wait_for(&self, ledger: u32) -> Result<()> {
        let following = ledger
            .checked_add(1)
            .ok_or_else(|| anyhow!("ledger {ledger} has no successor"))?;
        let needed = checkpoint_for(following);
        let started = Instant::now();

        loop {
            let current = self.current_ledger().await?;

            if current >= needed {
                tracing::info!(
                    ledger,
                    needed,
                    current,
                    "the archive holds the ledger and its successor"
                );

                return Ok(());
            }

            if started.elapsed() > self.checkpoint_timeout {
                bail!(
                    "the archive reached {current}, not checkpoint {needed}, within {}s",
                    self.checkpoint_timeout.as_secs()
                );
            }

            tracing::info!(
                ledger,
                needed,
                current,
                "waiting for the history archive checkpoint"
            );

            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    }

    pub async fn fixture(&self, ledger: u32) -> Result<Fixture> {
        let exported = export(ledger, &self.options, None)
            .await
            .map_err(|error| anyhow!("exporting ledger {ledger}: {error}"))?;
        let json = serde_json::to_string(&exported).context("serialising the exported ledger")?;

        Fixture::from_json(&json).context("reading the exported ledger as a prover fixture")
    }
}
