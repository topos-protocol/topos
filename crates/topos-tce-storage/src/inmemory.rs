use std::{
    collections::{BTreeMap, HashMap},
    time::{Instant, SystemTime},
};

use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{CertificateStatus, Height, InternalStorageError, PendingCertificateId, Storage, Tip};

pub struct InMemoryConfig {}

#[derive(Default)]
pub struct InMemoryStorage {
    pending_pool: BTreeMap<CertificateId, Certificate>,

    delivered_certs: HashMap<CertificateId, Certificate>,

    tips: BTreeMap<SubnetId, (CertificateId, Height)>,
}

impl InMemoryStorage {
    fn set_delivered(&mut self, certificate: Certificate) {
        let subnet = certificate.initial_subnet_id.clone();

        if let Some((ref mut certificate_id, ref mut height)) = self.tips.get_mut(&subnet) {
            *certificate_id = certificate.cert_id.clone();
            *height += 1;
        } else {
            self.tips.insert(subnet, (certificate.cert_id.clone(), 1));
        }

        self.delivered_certs
            .insert(certificate.cert_id.clone(), certificate);
    }
}
#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    async fn add_pending_certificate(
        &mut self,
        certificate: Certificate,
    ) -> Result<(), InternalStorageError> {
        self.pending_pool
            .insert(certificate.cert_id.clone(), certificate);

        Ok(())
    }

    async fn persist(
        &mut self,
        certificate: Certificate,
        status: CertificateStatus,
    ) -> Result<PendingCertificateId, InternalStorageError> {
        match status {
            CertificateStatus::Pending => {}
            CertificateStatus::Delivered => {
                // NOTE: Do we need to validate that the certificate was in the pending_pool?
                self.set_delivered(certificate);
            }
        }

        Ok(0)
    }

    async fn update(
        &mut self,
        certificate_id: &CertificateId,
        status: CertificateStatus,
    ) -> Result<(), InternalStorageError> {
        match status {
            CertificateStatus::Pending => {
                // NOTE: A certificate is by default in Pending. If it's validated we can't change
                // its status
                Ok(())
            }
            CertificateStatus::Delivered => {
                if let Some(certificate) = self.pending_pool.remove(certificate_id) {
                    self.set_delivered(certificate);

                    Ok(())
                } else {
                    Err(InternalStorageError::CertificateNotFound(
                        certificate_id.clone(),
                    ))
                }
            }
        }
    }

    async fn get_tip(&self, subnets: Vec<SubnetId>) -> Result<Vec<Tip>, InternalStorageError> {
        let mut result = Vec::new();

        for subnet in subnets {
            if let Some((certificate_id, height)) = self.tips.get(&subnet) {
                result.push(Tip {
                    cert_id: certificate_id.clone(),
                    subnet_id: subnet.clone(),
                    height: *height,
                    timestamp: SystemTime::now(),
                })
            }
        }

        Ok(result)
    }

    async fn get_certificates(
        &self,
        cert_ids: Vec<CertificateId>,
    ) -> Result<Vec<(CertificateStatus, Certificate)>, InternalStorageError> {
        let mut result = Vec::new();

        // NOTE: What to do if one cert is not found?
        for cert_id in cert_ids {
            if let Ok(certificate) = self.get_certificate(cert_id).await {
                result.push((CertificateStatus::Delivered, certificate));
            }
        }

        Ok(result)
    }

    async fn get_certificate(
        &self,
        cert_id: CertificateId,
    ) -> Result<Certificate, InternalStorageError> {
        if let Some(certificate) = self.delivered_certs.get(&cert_id) {
            Ok(certificate.clone())
        } else {
            Err(InternalStorageError::CertificateNotFound(cert_id))
        }
    }

    async fn get_emitted_certificates(
        &self,
        _subnet_id: SubnetId,
        _from: Height,
        _to: Height,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        todo!()
    }

    async fn get_received_certificates(
        &self,
        _subnet_id: SubnetId,
        _from: Instant,
        _to: Instant,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        todo!()
    }

    async fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(u64, Certificate)>, InternalStorageError> {
        todo!()
    }

    async fn remove_pending_certificate(&self, _index: u64) -> Result<(), InternalStorageError> {
        todo!()
    }
}
