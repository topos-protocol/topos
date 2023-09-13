use std::path::PathBuf;

use rocksdb::ColumnFamilyDescriptor;
use topos_core::uci::CertificateId;

use crate::{
    rocks::{
        db::{default_options, init_db, init_with_cfs},
        db_column::DBColumn,
    },
    types::{BroadcastState, EpochId, Participants, VerifiedCheckpointSummary},
};

pub struct EpochParticipantsTables {
    #[allow(unused)]
    participants_map: DBColumn<EpochId, Participants>,
}

impl EpochParticipantsTables {
    pub(crate) fn open(mut path: PathBuf) -> Self {
        path.push("participants");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        let db = init_db(&path, options).unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            participants_map: DBColumn::reopen(&db, "participants"),
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
    participants: Vec<Participants>,
}

impl ValidatorPerEpochTables {
    pub(crate) fn open(epoch_id: EpochId, mut path: PathBuf) -> Self {
        path.push("epochs");
        path.push(epoch_id.to_string());
        let cfs = vec![
            ColumnFamilyDescriptor::new("epoch_summary", default_options()),
            ColumnFamilyDescriptor::new("broadcast_states", default_options()),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            epoch_summary: DBColumn::reopen(&db, "epoch_summary"),
            broadcast_states: DBColumn::reopen(&db, "broadcast_states"),
            participants: Vec::new(),
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
