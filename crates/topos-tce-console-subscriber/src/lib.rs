mod aggregator;
mod builder;
mod command;
mod event;
mod layer;
mod server;
mod visitors;

pub fn init() {
    layer::ConsoleLayer::builder().with_default_env().init();
}
