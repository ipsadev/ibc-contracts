use anyhow::{anyhow, bail, Context, Result};
use prost::Message;
use tonic::{
    client::Grpc, codec::ProstCodec, codegen::http::uri::PathAndQuery, transport::Endpoint,
};

const TREE_SIBLINGS: &str = "/stellar.gateway.v1.StellarGatewayQuery/QueryTreeSiblings";

#[derive(Clone, PartialEq, Message)]
struct QueryTreeSiblingsRequest {
    #[prost(uint32, tag = "1")]
    ledger: u32,
    #[prost(bytes = "vec", tag = "2")]
    path: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
struct QueryTreeSiblingsResponse {
    #[prost(uint32, tag = "1")]
    ledger: u32,
    #[prost(bytes = "vec", tag = "2")]
    root: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    value: Vec<u8>,
    #[prost(bytes = "vec", repeated, tag = "4")]
    siblings: Vec<Vec<u8>>,
}

pub struct TreeSiblings {
    pub root: [u8; 32],
    pub value: Vec<u8>,
    pub siblings: Vec<[u8; 32]>,
}

pub async fn tree_siblings(address: &str, ledger: u32, path: &[u8]) -> Result<TreeSiblings> {
    let channel = Endpoint::from_shared(address.to_string())
        .with_context(|| format!("gateway address {address}"))?
        .connect()
        .await
        .with_context(|| format!("connecting to the gateway at {address}"))?;

    let mut grpc = Grpc::new(channel);

    grpc.ready()
        .await
        .map_err(|error| anyhow!("the gateway at {address} is not ready: {error}"))?;

    let response: QueryTreeSiblingsResponse = grpc
        .unary(
            tonic::Request::new(QueryTreeSiblingsRequest {
                ledger,
                path: path.to_vec(),
            }),
            PathAndQuery::from_static(TREE_SIBLINGS),
            ProstCodec::default(),
        )
        .await
        .map_err(|status| {
            anyhow!(
                "the gateway refused the siblings of ledger {ledger}: {}",
                status.message()
            )
        })?
        .into_inner();

    if response.ledger != ledger {
        bail!(
            "the gateway answered for ledger {}, not {ledger}",
            response.ledger
        );
    }

    let root = response
        .root
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("the gateway's root is not 32 bytes"))?;

    let siblings = response
        .siblings
        .iter()
        .map(|sibling| {
            sibling
                .as_slice()
                .try_into()
                .map_err(|_| anyhow!("a sibling from the gateway is not 32 bytes"))
        })
        .collect::<Result<Vec<[u8; 32]>>>()?;

    Ok(TreeSiblings {
        root,
        value: response.value,
        siblings,
    })
}
