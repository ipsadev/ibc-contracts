FROM rust:1.92-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    protobuf-compiler \
    libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY packages packages
COPY programs programs
COPY ibc-solidity/contracts ibc-solidity/contracts
COPY ibc-solidity/abi ibc-solidity/abi
COPY ibc-solidity/programs ibc-solidity/programs
COPY ibc-solana/Cargo.toml ibc-solana/Cargo.toml
COPY ibc-solana/packages ibc-solana/packages
COPY proto proto

RUN cargo build --bin stellar-proof-api --release --locked

FROM debian:bookworm-slim AS certs
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

FROM gcr.io/distroless/cc-debian12

ARG VCS_REF
LABEL org.opencontainers.image.title="stellar-proof-api" \
      org.opencontainers.image.description="Proof API for the Stellar and Ethereum corridors of ipsa" \
      org.opencontainers.image.revision=$VCS_REF

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

WORKDIR /app
COPY --from=builder /app/target/release/stellar-proof-api /usr/local/bin/stellar-proof-api
COPY --from=artifacts . /app/

EXPOSE 3100
ENTRYPOINT ["/usr/local/bin/stellar-proof-api", "start", "--config", "/app/proof-api.json"]
