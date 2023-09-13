use std::path::PathBuf;
use std::sync::Arc;
use std::{collections::HashMap, sync::RwLock};

use arc_swap::ArcSwap;

use crate::errors::StorageError;
use crate::types::{EpochId, Participants};

pub(crate) use self::tables::EpochParticipantsTables;
pub(crate) use self::tables::ValidatorPerEpochTables;

mod tables;

/// Epoch contextualized data - can be purged at some point
pub struct ValidatorPerEpochStore {
    #[allow(unused)]
    epoch_id: EpochId,
    #[allow(unused)]
    participants: RwLock<Vec<Participants>>,
    #[allow(unused)]
    tables: ValidatorPerEpochTables,
}

impl ValidatorPerEpochStore {
    pub fn new(epoch_id: EpochId, path: PathBuf) -> Result<ArcSwap<Self>, StorageError> {
        let tables: ValidatorPerEpochTables = ValidatorPerEpochTables::open(epoch_id, path);
        let store = ArcSwap::from(Arc::new(Self {
            epoch_id,
            participants: RwLock::new(Vec::new()),
            tables,
        }));

        Ok(store)
    }
}
pub struct EpochParticipantsStore {
    #[allow(unused)]
    tables: EpochParticipantsTables,
    #[allow(unused)]
    caches: RwLock<HashMap<EpochId, Participants>>,
}

impl EpochParticipantsStore {
    pub fn new(path: PathBuf) -> Result<Arc<Self>, StorageError> {
        let tables = EpochParticipantsTables::open(path);
        let store = Arc::new(Self {
            tables,
            caches: RwLock::new(HashMap::new()),
        });

        Ok(store)
    }
}
