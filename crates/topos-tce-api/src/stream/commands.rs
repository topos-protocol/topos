use topos_core::uci::Certificate;

pub enum StreamCommand {
    PushCertificate {
        subnet_id: String,
        certificate: Certificate,
    },
}
