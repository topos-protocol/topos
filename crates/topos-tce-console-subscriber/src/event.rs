use crate::visitors::{double_echo::DoubleEchoEvent, network::NetworkEvent, runtime::RuntimeEvent};

#[derive(Debug)]
pub enum Event {
    Network(NetworkEvent),
    Runtime(RuntimeEvent),
    DoubleEcho(DoubleEchoEvent),
}
