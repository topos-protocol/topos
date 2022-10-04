use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use rocksdb::{ColumnFamilyDescriptor, Options};
use topos_core::uci::{Certificate, CertificateId};

use crate::{
    errors::{InternalStorageError, StorageError},
    CertificateStatus, Height, Storage, Tip,
};

use self::db_column::DBColumn;

// Column family for all certificates
// Key: cert_id
// Value: (cert_status, cert_data)
// NOTE: No need to be kept sorted: point_lookup
const CERT_CF: &str = "certificates";
const PENDING_CERTIFICATES: &str = "PENDING_CERTIFICATES";
// Column family for the tips (last emitted cert per subnet)
// Key: subnet_id
// Value: (cert_id, height)
// NOTE: Separated to dedicated cf because dedicated workload
const TIPS_CF: &str = "tips";
// Column family for cert by receiver
// Key: (subnet_id, timestamp)
// Value: (cert_id)
const RECEIVER_CF: &str = "receivers";
// Column family for cert by emitter
// Key: (subnet_id, height)
// Value: (cert_id)
const EMITTER_CF: &str = "emitters";

mod db_column;

#[derive(Debug)]
pub enum RocksDBError {}

#[derive(Debug)]
pub struct RocksDBStorage {
    pending_certificates: DBColumn<u64, Certificate>,
    next_pending_id: AtomicU64,
}

impl RocksDBStorage {
    pub async fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            pending_certificates: DBColumn::open(path, None, PENDING_CERTIFICATES).unwrap(),
            next_pending_id: AtomicU64::new(0),
        })
        // fs::create_dir_all(path).map_err(|_| {
        //     StorageError::InternalStorage(InternalStorageError::UnableToStartStorage)
        // })?;
        //
        // let cf_opts = Options::default();
        // let cfs = vec![
        //     ColumnFamilyDescriptor::new(CERT_CF, cf_opts.clone()),
        //     ColumnFamilyDescriptor::new(TIPS_CF, cf_opts.clone()),
        //     ColumnFamilyDescriptor::new(RECEIVER_CF, cf_opts.clone()),
        //     ColumnFamilyDescriptor::new(EMITTER_CF, cf_opts.clone()),
        // ];
        //
        // let db_opts = Options::default();
        //
        // // Open the database
        // Ok(Self {
        //     db: rocksdb::DB::open_cf_descriptors(&db_opts, path, cfs).map_err(|_| {
        //         StorageError::InternalStorage(InternalStorageError::UnableToStartStorage)
        //     })?,
        // })
    }
}

#[async_trait::async_trait]
impl Storage for RocksDBStorage {
    async fn persist(
        &mut self,
        certificate: topos_core::uci::Certificate,
        status: crate::CertificateStatus,
    ) -> Result<(), crate::errors::InternalStorageError> {
        match status {
            CertificateStatus::Pending => self.pending_certificates.insert(
                &self.next_pending_id.fetch_add(1, Ordering::Relaxed),
                &certificate,
            ),
            CertificateStatus::Delivered => unimplemented!(),
        }
    }

    async fn update(
        &mut self,
        certificate_id: &topos_core::uci::CertificateId,
        status: crate::CertificateStatus,
    ) -> Result<(), crate::errors::InternalStorageError> {
        unimplemented!();
        // let tips_cf = self.db.cf_handle(TIPS_CF).unwrap();
        // let cert_cf = self.db.cf_handle(CERT_CF).unwrap();
        //
        // if let CertificateStatus::Delivered = status {
        //     if let Ok(Some(value)) = self.db.get_cf(&cert_cf, certificate_id) {
        //         if let Ok((old_status, cert)) =
        //             bincode::deserialize::<(CertificateStatus, Certificate)>(&value)
        //         {
        //             if old_status != status {
        //                 let new_value = bincode::serialize(&(status, &cert)).unwrap();
        //
        //                 self.db.put_cf(&cert_cf, certificate_id, new_value)?;
        //                 let new_height = match self.db.get_cf(&tips_cf, &cert.initial_subnet_id) {
        //                     Ok(Some(raw_tip)) => {
        //                         if let Ok((cert, height)) =
        //                             bincode::deserialize::<(CertificateId, Height)>(&raw_tip)
        //                         {
        //                             bincode::serialize(&(&cert, height + 1)).unwrap()
        //                         } else {
        //                             // TODO: Should be revert the status change ?
        //                             return Err(InternalStorageError::UnableToDeserializeValue);
        //                         }
        //                     }
        //                     Ok(None) => bincode::serialize(&(&cert, 1)).unwrap(),
        //                     Err(error) => return Err(InternalStorageError::RocksDBError(error)),
        //                 };
        //
        //                 Ok(self
        //                     .db
        //                     .put_cf(&tips_cf, &cert.initial_subnet_id, new_height)?)
        //             } else {
        //                 Ok(())
        //             }
        //         } else {
        //             Err(InternalStorageError::UnableToDeserializeValue)
        //         }
        //     } else {
        //         Err(InternalStorageError::CertificateNotFound(
        //             certificate_id.clone(),
        //         ))
        //     }
        // } else {
        //     Ok(())
        // }
    }

    async fn get_tip(
        &self,
        subnets: Vec<topos_core::uci::SubnetId>,
    ) -> Result<Vec<crate::Tip>, crate::errors::InternalStorageError> {
        unimplemented!();
        // let cf = self.db.cf_handle(TIPS_CF).unwrap();
        //
        // let key_bytes: Result<Vec<_>, InternalStorageError> =
        //     subnets.into_iter().map(|k| Ok((&cf, k))).collect();
        //
        // let result = self.db.multi_get_cf(key_bytes?);
        //
        // Ok(result
        //     .into_iter()
        //     .filter_map(|value_byte| match value_byte.ok()? {
        //         Some(data) => Some(bincode::deserialize::<Tip>(&data).unwrap()),
        //         None => None,
        //     })
        //     .collect())
    }

    async fn get_certificates(
        &self,
        cert_id: Vec<topos_core::uci::CertificateId>,
    ) -> Result<
        Vec<(crate::CertificateStatus, topos_core::uci::Certificate)>,
        crate::errors::InternalStorageError,
    > {
        unimplemented!();
        // let cf = self.db.cf_handle(CERT_CF).unwrap();
        //
        // let key_bytes: Result<Vec<_>, InternalStorageError> =
        //     cert_id.into_iter().map(|k| Ok((&cf, k))).collect();
        //
        // let result = self.db.multi_get_cf(key_bytes?);
        //
        // Ok(result
        //     .into_iter()
        //     .filter_map(|value_byte| match value_byte.ok()? {
        //         Some(data) => {
        //             Some(bincode::deserialize::<(CertificateStatus, Certificate)>(&data).unwrap())
        //         }
        //         None => None,
        //     })
        //     .collect())
    }

    async fn get_certificate(
        &self,
        cert_id: topos_core::uci::CertificateId,
    ) -> Result<
        (crate::CertificateStatus, topos_core::uci::Certificate),
        crate::errors::InternalStorageError,
    > {
        unimplemented!();
        // let cf = self.db.cf_handle(CERT_CF).unwrap();
        //
        // let result = self.db.get_cf(&cf, &cert_id)?;
        //
        // result
        //     .map(|data| bincode::deserialize(&data).unwrap())
        //     .ok_or(InternalStorageError::CertificateNotFound(cert_id))
    }

    async fn get_emitted_certificates(
        &self,
        subnet_id: topos_core::uci::SubnetId,
        from: crate::Height,
        to: crate::Height,
    ) -> Result<Vec<topos_core::uci::CertificateId>, crate::errors::InternalStorageError> {
        unimplemented!();
        // let cf = self.db.cf_handle(EMITTER_CF).unwrap();
        //
        // let range = if from > to {
        //     to..=from
        // } else if to > from {
        //     from..=to
        // } else {
        //     return Err(InternalStorageError::InvalidQueryArgument(
        //         "From and To can't be equal",
        //     ));
        // };
        //
        // let key_bytes: Result<Vec<_>, InternalStorageError> = range
        //     .into_iter()
        //     .map(|i| Ok((&cf, bincode::serialize(&(&subnet_id, i)).unwrap())))
        //     .collect();
        //
        // let result = self.db.multi_get_cf(key_bytes?);
        //
        // Ok(result
        //     .into_iter()
        //     .filter_map(|value_byte| match value_byte.ok()? {
        //         Some(data) => Some(bincode::deserialize(&data).unwrap()),
        //         None => None,
        //     })
        //     .collect())
    }

    async fn get_received_certificates(
        &self,
        subnet_id: topos_core::uci::SubnetId,
        from: std::time::Instant,
        to: std::time::Instant,
    ) -> Result<Vec<topos_core::uci::CertificateId>, crate::errors::InternalStorageError> {
        // NOTE: How to query for range of timestamp when everything is bytes in rocksdb? we need
        // to use raw_iterator_cf but how to process fields?
        unimplemented!()
    }

    async fn get_certificate_pending(
        &self,
    ) -> Result<Vec<topos_core::uci::CertificateId>, crate::errors::InternalStorageError> {
        todo!()
    }
}
