use std::time::Duration;

use alloy::{
    primitives::{keccak256, Address, TxHash, B256, U256},
    providers::{Provider, RootProvider},
    sol_types::SolEventInterface,
};
use anyhow::{anyhow, bail, Context, Result};
use ethereum_light_client::membership::MembershipProof;
use ethereum_types::execution::{account_proof::AccountProof, storage_proof::StorageProof};
use ibc_eureka_solidity_types::ics26::{router::routerEvents, IICS26RouterMsgs::Packet};
use sha2::{Digest, Sha256};

pub struct Evm {
    provider: RootProvider,
    router: Address,
    commitment_slot: U256,
}

pub enum Relayable {
    Receive(Packet),
    Acknowledge(Packet, Vec<u8>),
}

impl Relayable {
    pub fn packet(&self) -> &Packet {
        match self {
            Self::Receive(packet) | Self::Acknowledge(packet, _) => packet,
        }
    }

    pub fn evm_path(&self) -> Vec<u8> {
        let (client, discriminator) = match self {
            Self::Receive(packet) => (&packet.sourceClient, 1_u8),
            Self::Acknowledge(packet, _) => (&packet.destClient, 3_u8),
        };

        let mut path = client.as_bytes().to_vec();

        path.push(discriminator);
        path.extend_from_slice(&self.packet().sequence.to_be_bytes());

        path
    }

    pub fn expected_value(&self) -> Vec<u8> {
        match self {
            Self::Receive(packet) => packet.commitment(),
            Self::Acknowledge(_, acknowledgement) => {
                let mut preimage = vec![2_u8];

                preimage.extend_from_slice(&Sha256::digest(acknowledgement));

                Sha256::digest(&preimage).to_vec()
            }
        }
    }
}

pub struct Found {
    pub relayable: Relayable,
    pub block_number: u64,
}

impl Evm {
    pub async fn connect(url: &str, router: Address, commitment_slot: U256) -> Result<Self> {
        let provider = RootProvider::builder()
            .connect(url)
            .await
            .with_context(|| format!("connecting to {url}"))?;

        Ok(Self {
            provider,
            router,
            commitment_slot,
        })
    }

    pub const fn router(&self) -> Address {
        self.router
    }

    pub const fn commitment_slot(&self) -> U256 {
        self.commitment_slot
    }

    pub async fn chain_id(&self) -> Result<u64> {
        self.provider
            .get_chain_id()
            .await
            .context("reading the EVM chain id")
    }

    pub async fn relayable(
        &self,
        transaction: TxHash,
        evm_client: &str,
        stellar_client: &str,
        sequences: &[u64],
    ) -> Result<Found> {
        let receipt = self
            .provider
            .get_transaction_receipt(transaction)
            .await
            .context("reading the transaction receipt")?
            .ok_or_else(|| anyhow!("{transaction} has no receipt yet"))?;

        if !receipt.status() {
            bail!("{transaction} reverted");
        }

        let block_number = receipt
            .block_number
            .ok_or_else(|| anyhow!("{transaction} is not in a block yet"))?;
        let wanted = |packet: &Packet| sequences.is_empty() || sequences.contains(&packet.sequence);
        let mut found = Vec::new();

        for log in receipt.inner.logs() {
            if log.address() != self.router {
                continue;
            }

            let Ok(event) = routerEvents::decode_log(&log.inner) else {
                continue;
            };

            match event.data {
                routerEvents::SendPacket(sent)
                    if sent.packet.sourceClient == evm_client
                        && sent.packet.destClient == stellar_client
                        && wanted(&sent.packet) =>
                {
                    found.push(Relayable::Receive(sent.packet));
                }
                routerEvents::WriteAcknowledgement(written)
                    if written.packet.destClient == evm_client
                        && written.packet.sourceClient == stellar_client
                        && wanted(&written.packet) =>
                {
                    let acknowledgement = written
                        .acknowledgements
                        .first()
                        .ok_or_else(|| {
                            anyhow!("the acknowledgement event carries no acknowledgement")
                        })?
                        .to_vec();

                    found.push(Relayable::Acknowledge(written.packet, acknowledgement));
                }
                _ => {}
            }
        }

        match found.len() {
            1 => Ok(Found {
                relayable: found.remove(0),
                block_number,
            }),
            0 => bail!(
                "{transaction} holds no packet from {evm_client} to {stellar_client} and no acknowledgement for one"
            ),
            count => bail!(
                "{transaction} holds {count} relayable packets; relaying more than one per transaction is not built yet"
            ),
        }
    }

    pub fn storage_key(&self, path: &[u8]) -> B256 {
        let mut preimage = keccak256(path).to_vec();

        preimage.extend_from_slice(&self.commitment_slot.to_be_bytes::<32>());

        keccak256(preimage)
    }

    pub async fn membership_proof(
        &self,
        path: &[u8],
        block_number: u64,
    ) -> Result<MembershipProof> {
        let key = self.storage_key(path);
        let mut last_error = None;

        for attempt in 1..=10 {
            match self
                .provider
                .get_proof(self.router, vec![key])
                .block_id(block_number.into())
                .await
            {
                Ok(response) => {
                    let storage = response
                        .storage_proof
                        .first()
                        .ok_or_else(|| anyhow!("eth_getProof returned no storage proof"))?;

                    return Ok(MembershipProof {
                        account_proof: AccountProof {
                            storage_root: response.storage_hash,
                            proof: response.account_proof.clone(),
                        },
                        storage_proof: StorageProof {
                            key,
                            value: storage.value,
                            proof: storage.proof.clone(),
                        },
                    });
                }
                Err(error) => {
                    tracing::info!(attempt, %error, "eth_getProof refused; retrying");

                    last_error = Some(error);

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }

        bail!(
            "eth_getProof at block {block_number} kept failing: {}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_default()
        )
    }
}
