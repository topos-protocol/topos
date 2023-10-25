use std::{fs::create_dir_all, path::PathBuf};

use rocksdb::ColumnFamilyDescriptor;
use topos_core::uci::CertificateId;
use tracing::warn;

use crate::{
    constant::cfs,
    rocks::{
        db::{default_options, init_db, init_with_cfs},
        db_column::DBColumn,
    },
    types::{BroadcastState, EpochId, Validators, VerifiedCheckpointSummary},
};

pub struct EpochValidatorsTables {
    #[allow(unused)]
    validators_map: DBColumn<EpochId, Validators>,
}

impl EpochValidatorsTables {
    pub(crate) fn open(mut path: PathBuf) -> Self {
        path.push("validators");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        let db = init_db(&path, options).unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            validators_map: DBColumn::reopen(&db, cfs::VALIDATORS),
        }
    }
}

/// Epoch contextualized data - can be purged at some point
pub struct ValidatorPerEpochTables {
    #[allow(unused)]
    epoch_summary: DBColumn<EpochSummaryKey, EpochSummaryValue>,
    #[allow(unused)]
    broadcast_states: DBColumn<CertificateId, BroadcastState>,
    #[allow(unused)]
    validators: Vec<Validators>,
}

impl ValidatorPerEpochTables {
    pub(crate) fn open(epoch_id: EpochId, mut path: PathBuf) -> Self {
        path.push("epochs");
        path.push(epoch_id.to_string());
        if !path.exists() {
            warn!("Path {:?} does not exist, creating it", path);
            create_dir_all(&path).expect("Cannot create ValidatorPerEpochTables directory");
        }
        let cfs = vec![
            ColumnFamilyDescriptor::new(cfs::EPOCH_SUMMARY, default_options()),
            ColumnFamilyDescriptor::new(cfs::BROADCAST_STATES, default_options()),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            epoch_summary: DBColumn::reopen(&db, cfs::EPOCH_SUMMARY),
            broadcast_states: DBColumn::reopen(&db, cfs::BROADCAST_STATES),
            validators: Vec::new(),
        }
    }
}

#[allow(unused)]
enum EpochSummaryKey {
    EpochId,
    StartCheckpoint,
    EndCheckpoint,
}

#[allow(unused)]
enum EpochSummaryValue {
    EpochId(EpochId),
    StartCheckpoint(VerifiedCheckpointSummary),
    EndCheckpoint(VerifiedCheckpointSummary),
}
