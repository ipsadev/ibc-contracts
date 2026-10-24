use std::{
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use rustls::{crypto::ring, ClientConfig, RootCertStore};
use rustls_pki_types::{pem::PemObject, CertificateDer};
use sp1_sdk::network::proto::types::ProofRequest;
use tokio_postgres::{Client, Config};
use tokio_postgres_rustls::MakeRustlsConnect;

pub const DATABASE_URL_VARIABLE: &str = "PROOF_HISTORY_DB_URL";

const RDS_BUNDLE: &[u8] = include_bytes!("../certificates/rds-global-bundle.pem");

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS proof_requests (
    id BIGSERIAL PRIMARY KEY,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    module TEXT NOT NULL,
    src_chain TEXT NOT NULL,
    dst_chain TEXT NOT NULL,
    kind TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    sequences BIGINT[] NOT NULL,
    step INTEGER NOT NULL,
    program TEXT NOT NULL,
    prover TEXT NOT NULL,
    status TEXT NOT NULL,
    error TEXT,
    vkey TEXT,
    trusted_height BIGINT,
    target_height BIGINT,
    public_values BYTEA,
    proof BYTEA,
    request_id TEXT,
    request_tx_hash TEXT,
    fulfiller TEXT,
    cycles BIGINT,
    gas_used BIGINT,
    gas_limit BIGINT,
    gas_price BIGINT,
    base_fee TEXT,
    deduction_amount TEXT,
    refund_amount TEXT,
    requested_at TIMESTAMPTZ,
    fulfilled_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS proof_requests_tx_hash ON proof_requests (tx_hash);
CREATE UNIQUE INDEX IF NOT EXISTS proof_requests_request_id ON proof_requests (request_id)
    WHERE request_id IS NOT NULL;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'proof_requests' AND column_name = 'attempts') THEN
        ALTER TABLE proof_requests ADD COLUMN attempts INTEGER NOT NULL DEFAULT 1;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name = 'proof_requests' AND column_name = 'last_attempt_at') THEN
        ALTER TABLE proof_requests ADD COLUMN last_attempt_at TIMESTAMPTZ;
    END IF;
END $$;";

pub const RETRYABLE: [&str; 2] = ["failed", "waiting"];

const SCHEMA_LOCK: i64 = 0x7072_6f6f_6673;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkRequest {
    pub id: String,
    pub tx_hash: Option<String>,
    pub fulfiller: Option<String>,
    pub cycles: Option<u64>,
    pub gas_used: Option<u64>,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u64>,
    pub base_fee: Option<String>,
    pub deduction_amount: Option<String>,
    pub refund_amount: Option<String>,
    pub requested_at: Option<u64>,
    pub fulfilled_at: Option<u64>,
}

impl NetworkRequest {
    pub fn from_sdk(id: [u8; 32], record: Option<&ProofRequest>) -> Self {
        let mut request = Self {
            id: format!("0x{}", hex::encode(id)),
            ..Self::default()
        };

        let Some(record) = record else {
            return request;
        };

        request.tx_hash = non_empty_hex(&record.tx_hash);
        request.fulfiller = record.fulfiller.as_deref().and_then(non_empty_hex);
        request.cycles = record.cycles;
        request.gas_used = record.gas_used;
        request.gas_limit = Some(record.gas_limit).filter(|limit| *limit > 0);
        request.gas_price = record.gas_price;
        request.base_fee = record.base_fee.clone();
        request.deduction_amount = record.deduction_amount.clone();
        request.refund_amount = record.refund_amount.clone();
        request.requested_at = Some(record.created_at).filter(|at| *at > 0);
        request.fulfilled_at = record.fulfilled_at;

        request
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofEntry {
    pub module: String,
    pub src_chain: String,
    pub dst_chain: String,
    pub kind: String,
    pub tx_hash: String,
    pub sequences: Vec<u64>,
    pub step: u32,
    pub program: String,
    pub prover: String,
    pub status: String,
    pub error: Option<String>,
    pub vkey: Option<[u8; 32]>,
    pub trusted_height: Option<u64>,
    pub target_height: Option<u64>,
    pub public_values: Option<Vec<u8>>,
    pub proof: Option<Vec<u8>>,
    pub request: Option<NetworkRequest>,
}

pub fn normalise_tx_hash(hash: &str) -> String {
    hash.trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .to_ascii_lowercase()
}

#[derive(Clone, Debug, Default)]
pub struct ProofHistory {
    url: Option<String>,
}

impl ProofHistory {
    pub fn new(url: Option<String>) -> Self {
        Self {
            url: url.filter(|url| !url.trim().is_empty()),
        }
    }

    pub fn from_env() -> Self {
        Self::new(std::env::var(DATABASE_URL_VARIABLE).ok())
    }

    pub fn enabled(&self) -> bool {
        self.url.is_some()
    }

    pub async fn prepare(&self) -> Result<()> {
        let Some(url) = &self.url else {
            tracing::info!("{DATABASE_URL_VARIABLE} is unset; proofs are not recorded");

            return Ok(());
        };

        let mut client = connect(url).await?;
        let transaction = client
            .transaction()
            .await
            .context("starting the proof history schema transaction")?;

        transaction
            .execute("SELECT pg_advisory_xact_lock($1)", &[&SCHEMA_LOCK])
            .await
            .context("waiting for another process creating proof_requests")?;
        transaction
            .batch_execute(SCHEMA)
            .await
            .context("creating proof_requests")?;
        transaction
            .commit()
            .await
            .context("committing proof_requests")?;

        tracing::info!("recording every proof in proof_requests");

        Ok(())
    }

    pub async fn record(&self, entry: &ProofEntry) {
        let Some(url) = &self.url else {
            return;
        };

        if let Err(error) = insert(url, entry).await {
            tracing::warn!(
                %error,
                tx_hash = %entry.tx_hash,
                program = %entry.program,
                "could not record the proof"
            );
        }
    }
}

async fn insert(url: &str, entry: &ProofEntry) -> Result<()> {
    let mut client = connect(url).await?;
    let request = entry.request.clone().unwrap_or_default();
    let sequences: Vec<i64> = entry
        .sequences
        .iter()
        .map(|sequence| *sequence as i64)
        .collect();
    let tx_hash = normalise_tx_hash(&entry.tx_hash);

    let transaction = client
        .transaction()
        .await
        .context("starting a proof history transaction")?;

    if entry.request.is_none() {
        let previous = transaction
            .query_opt(
                "SELECT id, status FROM proof_requests \
                 WHERE tx_hash = $1 AND module = $2 AND kind = $3 AND program = $4 \
                 AND step = $5 AND sequences = $6 AND src_chain = $7 AND dst_chain = $8 \
                 ORDER BY id DESC LIMIT 1 FOR UPDATE",
                &[
                    &tx_hash,
                    &entry.module,
                    &entry.kind,
                    &entry.program,
                    &(entry.step as i32),
                    &sequences,
                    &entry.src_chain,
                    &entry.dst_chain,
                ],
            )
            .await
            .context("reading the previous attempt")?;

        if let Some(previous) =
            previous.filter(|row| RETRYABLE.contains(&row.get::<_, String>(1).as_str()))
        {
            transaction
                .execute(
                    "UPDATE proof_requests SET status = $2, error = $3, vkey = $4, \
                     trusted_height = $5, target_height = $6, public_values = $7, proof = $8, \
                     attempts = attempts + 1, last_attempt_at = now() WHERE id = $1",
                    &[
                        &previous.get::<_, i64>(0),
                        &entry.status,
                        &entry.error,
                        &entry.vkey.map(|vkey| format!("0x{}", hex::encode(vkey))),
                        &entry.trusted_height.map(|height| height as i64),
                        &entry.target_height.map(|height| height as i64),
                        &entry.public_values,
                        &entry.proof,
                    ],
                )
                .await
                .context("recording another attempt")?;

            return transaction
                .commit()
                .await
                .context("committing the proof history");
        }
    }

    transaction
        .execute(
            "INSERT INTO proof_requests (module, src_chain, dst_chain, kind, tx_hash, sequences, \
             step, program, prover, status, error, vkey, trusted_height, target_height, \
             public_values, proof, request_id, request_tx_hash, fulfiller, cycles, gas_used, \
             gas_limit, gas_price, base_fee, deduction_amount, refund_amount, requested_at, \
             fulfilled_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
             $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28) \
             ON CONFLICT (request_id) WHERE request_id IS NOT NULL DO NOTHING",
            &[
                &entry.module,
                &entry.src_chain,
                &entry.dst_chain,
                &entry.kind,
                &tx_hash,
                &sequences,
                &(entry.step as i32),
                &entry.program,
                &entry.prover,
                &entry.status,
                &entry.error,
                &entry.vkey.map(|vkey| format!("0x{}", hex::encode(vkey))),
                &entry.trusted_height.map(|height| height as i64),
                &entry.target_height.map(|height| height as i64),
                &entry.public_values,
                &entry.proof,
                &entry.request.as_ref().map(|request| request.id.clone()),
                &request.tx_hash,
                &request.fulfiller,
                &request.cycles.map(|cycles| cycles as i64),
                &request.gas_used.map(|gas| gas as i64),
                &request.gas_limit.map(|gas| gas as i64),
                &request.gas_price.map(|price| price as i64),
                &request.base_fee,
                &request.deduction_amount,
                &request.refund_amount,
                &request.requested_at.map(timestamp),
                &request.fulfilled_at.map(timestamp),
            ],
        )
        .await
        .context("inserting into proof_requests")?;

    transaction
        .commit()
        .await
        .context("committing the proof history")
}

async fn connect(url: &str) -> Result<Client> {
    let mut config: Config = url
        .parse()
        .context("parsing the proof history database url")?;

    if config.get_connect_timeout().is_none() {
        config.connect_timeout(CONNECT_TIMEOUT);
    }

    let (client, connection) = config
        .connect(tls())
        .await
        .context("connecting to the proof history database")?;

    tokio::spawn(async move {
        if let Err(error) = connection.await {
            tracing::debug!(%error, "proof history connection closed");
        }
    });

    Ok(client)
}

pub fn tls() -> MakeRustlsConnect {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

    let config = CONFIG.get_or_init(|| {
        let mut roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };

        let certificates = CertificateDer::pem_slice_iter(RDS_BUNDLE)
            .collect::<Result<Vec<_>, _>>()
            .expect("the embedded rds bundle is valid pem");

        roots.add_parsable_certificates(certificates);

        let config = ClientConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("ring supports the default protocol versions")
            .with_root_certificates(roots)
            .with_no_client_auth();

        Arc::new(config)
    });

    MakeRustlsConnect::new(config.as_ref().clone())
}

fn non_empty_hex(bytes: &[u8]) -> Option<String> {
    (!bytes.is_empty()).then(|| format!("0x{}", hex::encode(bytes)))
}

fn timestamp(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}
