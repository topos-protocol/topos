use proto::instrument::{runtime_update::Update, NewCertificate};
use topos_tce_console_api as proto;
use tracing::field::Visit;

#[derive(Debug)]
pub enum RuntimeEvent {
    NewCertificate { subnet_id: String, cert_id: String },
}

impl RuntimeEvent {
    pub fn to_proto(&self) -> proto::instrument::RuntimeUpdate {
        proto::instrument::RuntimeUpdate {
            update: match self {
                RuntimeEvent::NewCertificate { subnet_id, cert_id } => {
                    Some(Update::NewCertificate(NewCertificate {
                        cert_id: cert_id.clone(),
                        subnet_id: subnet_id.clone(),
                    }))
                }
            },
        }
    }
}

#[derive(PartialEq, Debug)]
enum RuntimeEventType {
    NewCertificate,
}

#[derive(Default, Debug)]
pub struct RuntimeVisitor {
    event: Option<RuntimeEventType>,
    subnet_id: Option<String>,
    cert_id: Option<String>,
}

impl RuntimeVisitor {
    fn check_message(&mut self, message: String) {
        match message.to_lowercase() {
            m if m.starts_with("a certificate has been submitted") => {
                self.event = Some(RuntimeEventType::NewCertificate)
            }
            _ => {}
        }
    }

    pub(crate) fn result(self) -> Option<RuntimeEvent> {
        match self.event {
            Some(event_type) => match event_type {
                RuntimeEventType::NewCertificate
                    if self.subnet_id.is_some() && self.cert_id.is_some() =>
                {
                    Some(RuntimeEvent::NewCertificate {
                        subnet_id: self.subnet_id.unwrap(),
                        cert_id: self.cert_id.unwrap(),
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl Visit for RuntimeVisitor {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.check_message(format!("{:?}", value)),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        match field.name() {
            "initial_subnet_id" => self.subnet_id = Some(value.into()),
            "cert_id" => self.cert_id = Some(value.into()),
            _ => {}
        }
    }
}
