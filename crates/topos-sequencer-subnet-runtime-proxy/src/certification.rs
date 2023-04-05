use crate::Error;
use std::collections::LinkedList;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_sequencer_types::{BlockInfo, SubnetEvent};
use tracing::error;

pub struct Certification {
    /// Last known certificate id for subnet
    pub last_certificate_id: Option<CertificateId>,
    /// Latest BLOCK_HISTORY_LENGTH blocks kept in memory
    pub finalized_blocks: LinkedList<BlockInfo>,
    /// Subnet id for which certificates are generated
    pub subnet_id: SubnetId,
    /// Type of verifier used
    pub verifier: u32,
    /// Key for signing certificates, currently secp256k1
    signing_key: Vec<u8>,
}

impl Debug for Certification {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certification instance").finish()
    }
}

impl Certification {
    pub const BLOCK_HISTORY_LENGTH: usize = 256;

    pub fn new(
        subnet_id: &SubnetId,
        source_head_certificate_id: Option<CertificateId>,
        verifier: u32,
        signing_key: Vec<u8>,
    ) -> Result<Arc<Mutex<Certification>>, crate::Error> {
        Ok(Arc::new(Mutex::from(Self {
            last_certificate_id: source_head_certificate_id,
            finalized_blocks: LinkedList::<BlockInfo>::new(),
            subnet_id: *subnet_id,
            verifier,
            signing_key,
        })))
    }

    /// Generation of Certificates
    pub(crate) async fn generate_certificates(&mut self) -> Result<Vec<Certificate>, Error> {
        let subnet_id = self.subnet_id;
        let mut generated_certificates = Vec::new();

        // For every block, create one certificate
        // This will change after MVP
        for block_info in &self.finalized_blocks {
            // Parse target subnets from events
            let mut target_subnets: Vec<SubnetId> = Vec::new();
            for event in &block_info.events {
                match event {
                    SubnetEvent::TokenSent {
                        target_subnet_id, ..
                    } => {
                        target_subnets.push(*target_subnet_id);
                    }
                    SubnetEvent::ContractCall {
                        target_subnet_id, ..
                    } => {
                        target_subnets.push(*target_subnet_id);
                    }
                    SubnetEvent::ContractCallWithToken {
                        target_subnet_id, ..
                    } => {
                        target_subnets.push(*target_subnet_id);
                    }
                }
            }

            // Get the id of the previous Certificate from local history
            let previous_cert_id: CertificateId = match self.last_certificate_id {
                Some(cert_id) => cert_id,
                None => {
                    error!("Ill-formed subnet history for {:?}", subnet_id);
                    return Err(Error::IllFormedSubnetHistory);
                }
            };

            // TODO: acquire proof
            let proof = Vec::new();

            let mut certificate = Certificate::new(
                previous_cert_id,
                subnet_id,
                block_info.state_root,
                block_info.tx_root_hash,
                &target_subnets,
                self.verifier,
                proof,
            )
            .map_err(|e| Error::CertificateGenerationError(e.to_string()))?;
            certificate
                .update_signature(self.get_signing_key())
                .map_err(Error::CertificateSigningError)?;
            generated_certificates.push(certificate);
        }

        // Check for inconsistencies
        let last_known_certificate_id = self
            .last_certificate_id
            .ok_or(Error::InvalidPreviousCertificateId)?;
        for new_cert in &generated_certificates {
            if last_known_certificate_id == new_cert.id {
                // This should not happen
                panic!("Same certificate generated multiple times: {new_cert:?}");
            }
        }

        // Set info about latest known certificate for subnet
        if let Some(generated_certificate) = generated_certificates.iter().last() {
            self.last_certificate_id = Some(generated_certificate.id);
        }

        // Remove processed blocks
        self.finalized_blocks.clear();

        Ok(generated_certificates)
    }

    pub fn get_signing_key(&self) -> &[u8] {
        self.signing_key.as_slice()
    }

    /// Expand short block history. Remove older blocks
    pub fn append_blocks(&mut self, blocks: Vec<BlockInfo>) {
        self.finalized_blocks.extend(blocks);
        while self.finalized_blocks.len() > Self::BLOCK_HISTORY_LENGTH {
            self.finalized_blocks.pop_front();
        }
    }
}
