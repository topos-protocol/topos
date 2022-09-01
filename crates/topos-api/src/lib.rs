#[path = "generated"]
pub mod tce {
    #[path = "topos.tce.v1.rs"]
    pub mod v1;
}

#[path = "."]
pub mod uci {
    #[path = "generated/topos.uci.v1.rs"]
    pub mod v1;
    #[path = "conversions/uci/v1/uci.rs"]
    pub mod v1_conversions;
}
