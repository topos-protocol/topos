use futures::stream::FuturesUnordered;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

/// Status of a sync task
///
/// When registering a stream, a sync task is started to fetch certificates from the storage
/// and push them to the stream.
pub(crate) enum TaskStatus {
    ///  The sync task is active and started running
    Running,
    /// The sync task failed and reported an error
    Error,
    /// The sync task exited gracefully and is done pushing certificates to the stream
    Done,
}

pub(crate) struct Task {
    status: TaskStatus,
    stream_id: Uuid,
}

pub(crate) type SyncTasks =
    FuturesUnordered<Pin<Box<dyn Future<Output = (Uuid, TaskStatus)> + Send + 'static>>>;
