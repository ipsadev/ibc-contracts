use anyhow::{anyhow, Result};
use proof_api_core::{
    api::{
        self, proof_api_service_client::ProofApiServiceClient,
        proof_api_service_server::ProofApiService,
    },
    modules::ProofApiModule,
};
use serde::{Deserialize, Serialize};
use tonic::{
    transport::{Channel, Endpoint},
    Request, Response, Status,
};

#[derive(Clone, Copy, Debug)]
pub struct ForwardProofApiModule;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ForwardConfiguration {
    pub target: String,
}

struct ForwardService {
    client: ProofApiServiceClient<Channel>,
    target: String,
}

impl ForwardService {
    fn client(&self) -> ProofApiServiceClient<Channel> {
        self.client.clone()
    }

    fn unreachable(&self, status: Status) -> Status {
        tracing::warn!(target = %self.target, %status, "the forwarded proof API refused or failed");

        status
    }
}

#[tonic::async_trait]
impl ProofApiService for ForwardService {
    async fn info(&self, request: Request<api::InfoRequest>) -> Result<Response<api::InfoResponse>, Status> {
        self.client()
            .info(request.into_inner())
            .await
            .map_err(|status| self.unreachable(status))
    }

    async fn relay_by_tx(
        &self,
        request: Request<api::RelayByTxRequest>,
    ) -> Result<Response<api::RelayByTxResponse>, Status> {
        self.client()
            .relay_by_tx(request.into_inner())
            .await
            .map_err(|status| self.unreachable(status))
    }

    async fn create_client(
        &self,
        request: Request<api::CreateClientRequest>,
    ) -> Result<Response<api::CreateClientResponse>, Status> {
        self.client()
            .create_client(request.into_inner())
            .await
            .map_err(|status| self.unreachable(status))
    }

    async fn update_client(
        &self,
        request: Request<api::UpdateClientRequest>,
    ) -> Result<Response<api::UpdateClientResponse>, Status> {
        self.client()
            .update_client(request.into_inner())
            .await
            .map_err(|status| self.unreachable(status))
    }
}

#[tonic::async_trait]
impl ProofApiModule for ForwardProofApiModule {
    fn name(&self) -> &'static str {
        "forward"
    }

    async fn create_service(&self, configuration: serde_json::Value) -> Result<Box<dyn ProofApiService>> {
        let configuration = serde_json::from_value::<ForwardConfiguration>(configuration)
            .map_err(|error| anyhow!("failed to parse the forward configuration: {error}"))?;

        let channel = Endpoint::from_shared(configuration.target.clone())
            .map_err(|error| anyhow!("{} is not a usable address: {error}", configuration.target))?
            .connect_lazy();

        tracing::info!(target = %configuration.target, "forwarding this chain pair to another proof API");

        Ok(Box::new(ForwardService {
            client: ProofApiServiceClient::new(channel),
            target: configuration.target,
        }))
    }
}
