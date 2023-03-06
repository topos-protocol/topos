use crate::{Errors, TceStore};
use std::collections::{BTreeSet, HashMap};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

/// Store implementation in RAM good enough for functional tests
/// Might need to split through a new layer of TCEState
/// between ReliableBroadcast and rocksdb
#[derive(Default, Clone)]
pub struct TceMemStore {
    /// Global map of delivered and accepted certificates
    all_certs: HashMap<CertificateId, Certificate>,
    /// Mapping SubnetId -> Delivered certificated
    history: HashMap<SubnetId, BTreeSet<CertificateId>>,
    /// Consider for now that each TCE nodes is following all subnets
    tracked_digest: HashMap<SubnetId, BTreeSet<CertificateId>>,
    /// List of the subnets that we're part of
    /// NOTE: Below is for later, for now we're considering
    /// being part of all subnets, so following digest of everyone
    followed_subnet: Vec<SubnetId>,
}

impl TceMemStore {
    pub fn new(subnets: Vec<SubnetId>) -> TceMemStore {
        let mut store = TceMemStore {
            all_certs: Default::default(),
            history: Default::default(),
            tracked_digest: Default::default(),
            followed_subnet: subnets,
        };
        for subnet in &store.followed_subnet {
            store.tracked_digest.insert(*subnet, BTreeSet::new());
            store.history.insert(*subnet, BTreeSet::new());
        }
        // Add the genesis
        store
            .all_certs
            .insert(CertificateId::from_array([0u8; 32]), Default::default());
        store
    }
}

impl TceStore for TceMemStore {
    // JAEGER START DELIVERY TRACE [ cert, peer ]
    fn apply_cert(&mut self, cert: &Certificate) -> Result<(), Errors> {
        // Add the entry in the history <SubnetId, CertId>
        let _ = self.add_cert_in_hist(&cert.source_subnet_id, cert);

        // Add the cert into the history of each Target
        for target_subnet_id in &cert.target_subnets {
            self.add_cert_in_hist(target_subnet_id, cert);
            self.add_cert_in_digest(target_subnet_id, &cert.id);
        }

        Ok(())
    }

    fn add_cert_in_hist(&mut self, subnet_id: &SubnetId, cert: &Certificate) -> bool {
        self.all_certs.insert(cert.id, cert.clone());
        self.history.entry(*subnet_id).or_default().insert(cert.id)
    }

    fn add_cert_in_digest(&mut self, subnet_id: &SubnetId, cert_id: &CertificateId) -> bool {
        self.tracked_digest
            .entry(*subnet_id)
            .or_default()
            .insert(*cert_id)
    }

    fn read_journal(
        &self,
        _subnet_id: SubnetId,
        _from_offset: u64,
        _max_results: u64,
    ) -> Result<(Vec<Certificate>, u64), Errors> {
        unimplemented!();
    }

    fn recent_certificates_for_subnet(
        &self,
        subnet_id: &SubnetId,
        _last_n: u64,
    ) -> Vec<CertificateId> {
        match self.history.get(subnet_id) {
            Some(subnet_certs) => subnet_certs.iter().cloned().collect::<Vec<_>>(),
            None => Vec::new(),
        }
    }

    fn cert_by_id(&self, cert_id: &CertificateId) -> Result<Certificate, Errors> {
        match self.all_certs.get(cert_id) {
            Some(cert) => Ok(cert.clone()),
            _ => Err(Errors::CertificateNotFound),
        }
    }

    fn check_precedence(&self, cert: &Certificate) -> Result<(), Errors> {
        if cert.prev_id.as_array() == &[0u8; 32] {
            return Ok(());
        }
        match self.cert_by_id(&cert.prev_id) {
            Ok(_) => Ok(()),
            _ => Err(Errors::CertificateNotFound),
        }
    }

    fn clone_box(&self) -> Box<dyn TceStore + Send> {
        Box::new(self.clone())
    }
}
