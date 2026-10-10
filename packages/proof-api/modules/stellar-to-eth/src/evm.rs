use alloy::{
    primitives::{keccak256, Address},
    providers::{Provider, RootProvider},
    sol,
    sol_types::SolValue,
};
use anyhow::{anyhow, Context, Result};
use stellar_light_client_solidity_types::{ClientState, ConsensusState};

sol! {
    #[sol(rpc)]
    interface IClientDirectory {
        function getClient(string clientId) external view returns (address);
        function getCommitment(bytes32 hashedPath) external view returns (bytes32);
    }

    #[sol(rpc)]
    interface IStellarClientReader {
        function getClientState() external view returns (bytes memory);
        function getConsensusStateHash(uint64 revisionHeight) external view returns (bytes32);
    }
}

pub struct Evm {
    provider: RootProvider,
    router: Address,
}

impl Evm {
    pub async fn connect(url: &str, router: Address) -> Result<Self> {
        let provider = RootProvider::builder()
            .connect(url)
            .await
            .with_context(|| format!("connecting to {url}"))?;

        Ok(Self { provider, router })
    }

    pub const fn router(&self) -> Address {
        self.router
    }

    pub async fn chain_id(&self) -> Result<u64> {
        self.provider
            .get_chain_id()
            .await
            .context("reading the EVM chain id")
    }

    pub async fn client(&self, client_id: &str) -> Result<Address> {
        IClientDirectory::new(self.router, &self.provider)
            .getClient(client_id.to_string())
            .call()
            .await
            .with_context(|| format!("the router has no client {client_id}"))
    }

    pub async fn commitment(&self, path: &[u8]) -> Result<[u8; 32]> {
        let stored = IClientDirectory::new(self.router, &self.provider)
            .getCommitment(keccak256(path))
            .call()
            .await
            .context("reading a commitment from the router")?;

        Ok(stored.0)
    }

    pub async fn client_state(&self, client: Address) -> Result<ClientState> {
        let raw = IStellarClientReader::new(client, &self.provider)
            .getClientState()
            .call()
            .await
            .context("reading the Stellar client state")?;

        ClientState::abi_decode(&raw).map_err(|error| anyhow!("decoding the client state: {error}"))
    }

    pub async fn consensus_hash(&self, client: Address, height: u64) -> Result<[u8; 32]> {
        let hash = IStellarClientReader::new(client, &self.provider)
            .getConsensusStateHash(height)
            .call()
            .await
            .with_context(|| format!("the client holds no consensus state at ledger {height}"))?;

        Ok(hash.0)
    }
}

pub fn consensus_hash_of(consensus_state: &ConsensusState) -> [u8; 32] {
    keccak256(consensus_state.abi_encode()).0
}
