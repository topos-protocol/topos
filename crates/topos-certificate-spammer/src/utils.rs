use topos_core::uci::{Certificate, SubnetId};

use crate::{error::Error, SourceSubnet};

lazy_static::lazy_static! {
    static ref PROOF_SIZE: usize = std::env::var("PROOF_SIZE_KB").map(|v| v.parse::<usize>().unwrap_or(0)).unwrap_or(0) * 1024;
}

pub fn generate_random_32b_array() -> [u8; 32] {
    (0..32)
        .map(|_| rand::random::<u8>())
        .collect::<Vec<u8>>()
        .try_into()
        .expect("Valid 32 byte array")
}

/// Generate test certificate
pub fn generate_test_certificate(
    source_subnet: &mut SourceSubnet,
    target_subnet_ids: &[SubnetId],
) -> Result<Certificate, Box<dyn std::error::Error>> {
    let mut new_cert = Certificate::new(
        source_subnet.last_certificate_id,
        source_subnet.source_subnet_id,
        generate_random_32b_array(),
        generate_random_32b_array(),
        target_subnet_ids,
        0,
        vec![254u8; *PROOF_SIZE],
    )?;
    new_cert
        .update_signature(&source_subnet.signing_key)
        .map_err(Error::CertificateSigning)?;

    source_subnet.last_certificate_id = new_cert.id;
    Ok(new_cert)
}

pub fn generate_source_subnets(
    local_key_seed: u64,
    number_of_subnets: u8,
) -> Result<Vec<SourceSubnet>, Error> {
    let mut subnets = Vec::new();

    let mut signing_key = [0u8; 32];
    let (_, right) = signing_key.split_at_mut(24);
    right.copy_from_slice(local_key_seed.to_be_bytes().as_slice());
    for _ in 0..number_of_subnets {
        signing_key = tiny_keccak::keccak256(&signing_key);

        // Subnet id of the source subnet which will be used for every generated certificate
        let source_subnet_id: SubnetId = topos_crypto::keys::derive_public_key(&signing_key)
            .map_err(|e| Error::InvalidSigningKey(e.to_string()))?
            .as_slice()[1..33]
            .try_into()
            .map_err(|_| Error::InvalidSubnetId("Unable to parse subnet id".to_string()))?;

        subnets.push(SourceSubnet {
            signing_key,
            source_subnet_id,
            last_certificate_id: Default::default(),
        });
    }

    Ok(subnets)
}
