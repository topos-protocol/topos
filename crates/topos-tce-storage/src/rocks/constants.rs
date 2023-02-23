//! This module is defining constant names for CFs

pub(crate) const PENDING_CERTIFICATES: &str = "PENDING_CERTIFICATES";
pub(crate) const CERTIFICATES: &str = "CERTIFICATES";

pub(crate) const SOURCE_STREAMS: &str = "SOURCE_STREAMS";
pub(crate) const TARGET_STREAMS: &str = "TARGET_STREAMS";
pub(crate) const TARGET_SOURCES: &str = "TARGET_SOURCES";

pub(crate) const TARGET_STREAMS_PREFIX_SIZE: usize = 32 * 2;
pub(crate) const SOURCE_STREAMS_PREFIX_SIZE: usize = 32;
