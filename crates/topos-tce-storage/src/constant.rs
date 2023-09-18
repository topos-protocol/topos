pub(crate) mod cfs {
    pub(crate) const CERTIFICATES: &str = "certificates";
    pub(crate) const STREAMS: &str = "streams";
    pub(crate) const EPOCH_CHAIN: &str = "epoch_chain";
    pub(crate) const UNVERIFIED: &str = "unverified";

    pub(crate) const PENDING_POOL: &str = "pending_pool";
    pub(crate) const PENDING_POOL_INDEX: &str = "pending_pool_index";
    pub(crate) const PRECEDENCE_POOL: &str = "precedence_pool";

    pub(crate) const TARGET_STREAMS: &str = "target_streams";
    pub(crate) const TARGET_SOURCE_LIST: &str = "target_source_list";
    pub(crate) const SOURCE_LIST: &str = "source_list";
    pub(crate) const DELIVERED_CERTIFICATES_PER_SOURCE_FOR_TARGET: &str =
        "delivered_certificates_per_source_for_target";

    pub(crate) const VALIDATORS: &str = "validators";

    pub(crate) const EPOCH_SUMMARY: &str = "epoch_summary";
    pub(crate) const BROADCAST_STATES: &str = "broadcast_states";
}
