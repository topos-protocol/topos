use topos_core::uci::Certificate;

pub enum StreamCommand {
    PushCertificate { certificate: Certificate },
}
