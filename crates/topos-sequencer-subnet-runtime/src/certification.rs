use crate::Error;
use std::collections::{HashSet, LinkedList};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_sequencer_subnet_client::{BlockInfo, SubnetEvent};
use tracing::debug;

pub struct Certification {
    /// Last known certificate id for subnet
    pub last_certificate_id: Option<CertificateId>,
    /// Subnet id for which certificates are generated
    pub subnet_id: SubnetId,
    /// Type of verifier used
    pub verifier: u32,
    /// Key for signing certificates, currently secp256k1
    signing_key: Vec<u8>,
    /// Optional synchronization from particular block number
    pub start_block: Option<u64>,
    /// Latest BLOCK_HISTORY_LENGTH blocks kept in memory
    finalized_blocks: LinkedList<BlockInfo>,
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
        start_block: Option<u64>,
    ) -> Result<Arc<Mutex<Certification>>, crate::Error> {
        Ok(Arc::new(Mutex::from(Self {
            last_certificate_id: source_head_certificate_id,
            finalized_blocks: LinkedList::<BlockInfo>::new(),
            subnet_id: *subnet_id,
            verifier,
            signing_key,
            start_block,
        })))
    }

    /// Generation of Certificates
    pub(crate) async fn generate_certificates(&mut self) -> Result<Vec<Certificate>, Error> {
        let subnet_id = self.subnet_id;
        let mut generated_certificates = Vec::new();

        // Keep account of blocks with generated certificates so that we can remove them from
        // finalized blocks
        let mut processed_blocks: Vec<u64> = Vec::with_capacity(self.finalized_blocks.len());

        // For every block, create one certificate
        for block_info in &self.finalized_blocks {
            // Parse target subnets from events
            let mut target_subnets: HashSet<SubnetId> = HashSet::new();
            for event in &block_info.events {
                match event {
                    SubnetEvent::CrossSubnetMessageSent {
                        target_subnet_id, ..
                    } => {
                        target_subnets.insert(*target_subnet_id);
                    }
                }
            }

            // Get the id of the previous Certificate from local history
            let previous_cert_id: CertificateId = match self.last_certificate_id {
                Some(cert_id) => cert_id,
                None => {
                    // FIXME: This is genesis certificate we are generating because we are unable
                    // to retrieve one from TCE yet
                    CertificateId::default()
                }
            };

            // TODO: acquire proof
            let proof = Vec::new();

            let mut certificate = Certificate::new(
                previous_cert_id,
                subnet_id,
                block_info.state_root,
                block_info.tx_root_hash,
                block_info.receipts_root_hash,
                &target_subnets.into_iter().collect::<Vec<_>>(),
                self.verifier,
                proof,
            )
            .map_err(|e| Error::CertificateGenerationError(e.to_string()))?;
            certificate
                .update_signature(self.get_signing_key())
                .map_err(Error::CertificateSigningError)?;
            generated_certificates.push(certificate);
            processed_blocks.push(block_info.number);
        }

        // Check for inconsistencies
        let is_genesis_certificate: bool = self
            .finalized_blocks
            .front()
            .map(|b| b.number == 0 || self.start_block.is_some())
            .unwrap_or(false);
        let last_known_certificate_id = if is_genesis_certificate {
            // We are creating genesis certificate, there were no previous certificates
            // In case where start block is present, we also consider start block as genesis certificate,
            // so it has no history (prev cert id all 0)
            CertificateId::default()
        } else {
            self.last_certificate_id
                .ok_or(Error::InvalidPreviousCertificateId)?
        };

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
        for processed_block_number in processed_blocks {
            let mut front_block_number = None;
            if let Some(front) = self.finalized_blocks.front() {
                front_block_number = Some(front.number);
            }

            if front_block_number.is_some() {
                if Some(processed_block_number) == front_block_number {
                    debug!(
                        "Block {processed_block_number} processed and removed from the block list"
                    );
                    self.finalized_blocks.pop_front();
                } else {
                    panic!(
                        "Block history is inconsistent, this should not happen! \
                         processed_block_number: {processed_block_number}, front_number: {:?}",
                        front_block_number
                    );
                }
            }
        }

        Ok(generated_certificates)
    }

    pub fn get_signing_key(&self) -> &[u8] {
        self.signing_key.as_slice()
    }

    /// Expand short block history. Remove older blocks
    pub fn append_blocks(&mut self, blocks: Vec<BlockInfo>) {
        self.finalized_blocks.extend(blocks);
    }
}
