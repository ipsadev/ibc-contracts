// SPDX-License-Identifier: Apache-2.0

use alloy_primitives::B256;
use bls12_381::hash_to_curve::ExpandMsgXmd;
use bls12_381::hash_to_curve::HashToCurve;
use bls12_381::pairing;
use bls12_381::G1Affine;
use bls12_381::G1Projective;
use bls12_381::G2Affine;
use bls12_381::G2Projective;
use ethereum_light_client::verify::BlsVerify;
use ethereum_types::consensus::bls::BlsPublicKey;
use ethereum_types::consensus::bls::BlsSignature;
use ethereum_types::consensus::bls::BLS_DST_SIG;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BlsError {
    #[error("no public keys were supplied to aggregate")]
    NoPublicKeys,

    #[error("public key {index} is not a valid compressed G1 point")]
    MalformedPublicKey { index: usize },

    #[error("the signature is not a valid compressed G2 point")]
    MalformedSignature,

    #[error("the aggregate signature does not verify against the aggregate public key")]
    SignatureMismatch,
}

pub struct Bls12381Verifier;

impl BlsVerify for Bls12381Verifier {
    type Error = BlsError;

    fn fast_aggregate_verify(
        &self,
        public_keys: &[BlsPublicKey],
        msg: B256,
        signature: BlsSignature,
    ) -> Result<(), Self::Error> {
        let aggregate = aggregate_points(public_keys)?;

        let point = G2Affine::from_compressed(&signature.0)
            .into_option()
            .ok_or(BlsError::MalformedSignature)?;

        let message = <G2Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(
            [msg.as_slice()],
            BLS_DST_SIG,
        );

        // e(aggregate, H(m)) == e(g1, signature), the pairing form of fast_aggregate_verify.
        let left = pairing(&G1Affine::from(aggregate), &G2Affine::from(message));
        let right = pairing(&G1Affine::generator(), &point);

        if left == right {
            Ok(())
        } else {
            Err(BlsError::SignatureMismatch)
        }
    }

    fn aggregate(&self, public_keys: &[BlsPublicKey]) -> Result<BlsPublicKey, Self::Error> {
        let aggregate = aggregate_points(public_keys)?;

        Ok(BlsPublicKey::from(G1Affine::from(aggregate).to_compressed()))
    }
}

fn aggregate_points(public_keys: &[BlsPublicKey]) -> Result<G1Projective, BlsError> {
    if public_keys.is_empty() {
        return Err(BlsError::NoPublicKeys);
    }

    let mut total = G1Projective::identity();

    for (index, key) in public_keys.iter().enumerate() {
        let point = G1Affine::from_compressed(&key.0)
            .into_option()
            .ok_or(BlsError::MalformedPublicKey { index })?;

        total += G1Projective::from(point);
    }

    Ok(total)
}
