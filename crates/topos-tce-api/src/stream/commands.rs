use topos_core::uci::Certificate;

#[derive(Debug)]
pub enum StreamCommand {
    PushCertificate { certificate: Certificate },
}
