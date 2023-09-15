pub(crate) mod cfs {
    pub(crate) const CERTIFICATES: &'static str = "certificates";
    pub(crate) const STREAMS: &'static str = "streams";
    pub(crate) const EPOCH_CHAIN: &'static str = "epoch_chain";
    pub(crate) const UNVERIFIED: &'static str = "unverified";

    pub(crate) const PENDING_POOL: &'static str = "pending_pool";
    pub(crate) const PENDING_POOL_INDEX: &'static str = "pending_pool_index";
    pub(crate) const PRECEDENCE_POOL: &'static str = "precedence_pool";

    pub(crate) const TARGET_STREAMS: &'static str = "target_streams";
    pub(crate) const TARGET_SOURCE_LIST: &'static str = "target_source_list";
    pub(crate) const SOURCE_LIST: &'static str = "source_list";
    pub(crate) const DELIVERED_CERTIFICATES_PER_SOURCE_FOR_TARGET: &'static str =
        "delivered_certificates_per_source_for_target";

    pub(crate) const VALIDATORS: &'static str = "validators";

    pub(crate) const EPOCH_SUMMARY: &'static str = "epoch_summary";
    pub(crate) const BROADCAST_STATES: &'static str = "broadcast_states";
}
