use std::collections::{HashMap, HashSet};

use proto::instrument::{double_echo_update::Update, NewSample, Sample};
use tracing::field::Visit;

use topos_tce_console_api as proto;

#[derive(Debug)]
pub enum DoubleEchoEvent {
    NewSample {
        sample: HashMap<String, HashSet<String>>,
    },
}

impl DoubleEchoEvent {
    pub fn to_proto(&self) -> proto::instrument::DoubleEchoUpdate {
        proto::instrument::DoubleEchoUpdate {
            update: match self {
                Self::NewSample { sample } => Some(Update::NewSample(NewSample {
                    sample: Some(Sample {
                        echo_subscribers: sample
                            .get("EchoSubscriber")
                            .map(|v| v.iter().cloned().collect())
                            .unwrap_or_default(),
                        ready_subscribers: sample
                            .get("ReadySubscriber")
                            .map(|v| v.iter().cloned().collect())
                            .unwrap_or_default(),
                        echo_subsriptions: sample
                            .get("EchoSubscription")
                            .map(|v| v.iter().cloned().collect())
                            .unwrap_or_default(),
                        ready_subsriptions: sample
                            .get("ReadySubscription")
                            .map(|v| v.iter().cloned().collect())
                            .unwrap_or_default(),
                        delivery_subsriptions: sample
                            .get("DeliverySubscription")
                            .map(|v| v.iter().cloned().collect())
                            .unwrap_or_default(),
                    }),
                })),
            },
        }
    }
}
#[derive(PartialEq, Debug)]
pub enum DoubleEchoEventType {
    NewSample,
}

#[derive(Default, Debug)]
pub struct DoubleEchoVisitor {
    event: Option<DoubleEchoEventType>,
    sample: Option<HashMap<String, HashSet<String>>>,
}

impl DoubleEchoVisitor {
    fn check_message(&mut self, message: String) {
        match message.to_lowercase() {
            m if m.starts_with("new sample") => self.event = Some(DoubleEchoEventType::NewSample),
            _ => {}
        }
    }

    pub(crate) fn result(self) -> Option<DoubleEchoEvent> {
        println!("{self:?}");
        match self.event {
            Some(event_type) => match event_type {
                DoubleEchoEventType::NewSample if self.sample.is_some() => {
                    Some(DoubleEchoEvent::NewSample {
                        sample: self.sample.unwrap(),
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl Visit for DoubleEchoVisitor {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.check_message(format!("{:?}", value)),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        match field.name() {
            "sample" => self.sample = serde_json::from_str(value).ok(),
            _ => {}
        }
    }
}
