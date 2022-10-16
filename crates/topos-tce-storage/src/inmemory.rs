use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::{AtomicU64, Ordering},
    time::{Instant, SystemTime},
};

use tokio::sync::{Mutex, MutexGuard};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{CertificateStatus, Height, InternalStorageError, PendingCertificateId, Storage, Tip};

pub struct InMemoryConfig {}

pub struct InMemoryStorage {
    pending_pool: Mutex<BTreeMap<CertificateId, Certificate>>,

    delivered_certs: Mutex<HashMap<CertificateId, Certificate>>,

    tips: Mutex<BTreeMap<SubnetId, (CertificateId, Height)>>,

    next_pending_id: AtomicU64,
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self {
            pending_pool: Default::default(),
            delivered_certs: Default::default(),
            tips: Default::default(),
            next_pending_id: AtomicU64::new(0),
        }
    }
}

impl InMemoryStorage {
    async fn set_delivered(&self, certificate: Certificate) {
        let subnet = certificate.initial_subnet_id.clone();

        let mut tips = self.tips.lock().await;
        if let Some((ref mut certificate_id, ref mut height)) = tips.get_mut(&subnet) {
            *certificate_id = certificate.cert_id.clone();
            *height += 1;
        } else {
            tips.insert(subnet, (certificate.cert_id.clone(), 1));
        }

        self.delivered_certs
            .lock()
            .await
            .insert(certificate.cert_id.clone(), certificate);
    }

    fn get_certificate_with_lock(
        &self,
        cert_id: CertificateId,
        lock: &MutexGuard<'_, HashMap<CertificateId, Certificate>>,
    ) -> Result<Certificate, InternalStorageError> {
        if let Some(certificate) = lock.get(&cert_id) {
            Ok(certificate.clone())
        } else {
            Err(InternalStorageError::CertificateNotFound(cert_id))
        }
    }
}

#[async_trait::async_trait]
impl Storage for InMemoryStorage {
    async fn add_pending_certificate(
        &self,
        certificate: Certificate,
    ) -> Result<PendingCertificateId, InternalStorageError> {
        let key = self.next_pending_id.fetch_add(1, Ordering::Relaxed);

        self.pending_pool
            .lock()
            .await
            .insert(key.to_string(), certificate);

        Ok(key)
    }

    async fn persist(
        &self,
        certificate: Certificate,
        status: CertificateStatus,
    ) -> Result<PendingCertificateId, InternalStorageError> {
        match status {
            CertificateStatus::Pending => {}
            CertificateStatus::Delivered => {
                // NOTE: Do we need to validate that the certificate was in the pending_pool?
                self.set_delivered(certificate).await;
            }
        }

        Ok(0)
    }

    async fn update(
        &self,
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
                if let Some(certificate) = self.pending_pool.lock().await.remove(certificate_id) {
                    self.set_delivered(certificate).await;

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

        let tips = self.tips.lock().await;

        for subnet in subnets {
            if let Some((certificate_id, height)) = tips.get(&subnet) {
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
        certificate_ids: Vec<CertificateId>,
    ) -> Result<Vec<Certificate>, InternalStorageError> {
        let mut result = Vec::new();

        let lock = self.delivered_certs.lock().await;

        // NOTE: What to do if one cert is not found?
        for cert_id in certificate_ids {
            if let Ok(certificate) = self.get_certificate_with_lock(cert_id, &lock) {
                result.push(certificate);
            }
        }

        Ok(result)
    }

    async fn get_certificate(
        &self,
        cert_id: CertificateId,
    ) -> Result<Certificate, InternalStorageError> {
        self.get_certificate_with_lock(cert_id, &self.delivered_certs.lock().await)
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
