use std::collections::HashMap;

use console_api::instrument::{
    double_echo_update, network_update, runtime_update, ConnectedPeer, DisconnectedPeer,
    NewCertificate, NewSample, WatchUpdatesResponse,
};
use crossterm::event::{Event, KeyCode, KeyEvent};
use tui::widgets::ListState;

pub struct App<'a> {
    pub titles: Vec<&'a str>,
    pub index: usize,
    pub network: HashMap<String, ConnectedPeer>,
    pub certificates: HashMap<String, NewCertificate>,
    pub samples: HashMap<String, Vec<String>>,
    pub current_selected_sample: ListState,
}

impl<'a> App<'a> {
    pub fn new() -> App<'a> {
        App {
            titles: vec!["Network", "Certificates", "Streams", "Sample"],
            index: 0,
            network: HashMap::new(),
            certificates: HashMap::new(),
            samples: HashMap::new(),
            current_selected_sample: ListState::default(),
        }
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.titles.len();
    }

    pub fn previous(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.titles.len() - 1;
        }
    }

    pub fn handle_input(&mut self, input: Event) -> Option<()> {
        if let Event::Key(KeyEvent { code, .. }) = input {
            match code {
                KeyCode::Char('q') => return Some(()),
                KeyCode::Right => self.next(),
                KeyCode::Left => self.previous(),
                KeyCode::Down => {
                    let index = match self.current_selected_sample.selected() {
                        Some(index) => index + 1,
                        None => 0,
                    };

                    if index > 4 {
                        self.current_selected_sample.select(None);
                    } else {
                        self.current_selected_sample.select(Some(index));
                    }
                }
                KeyCode::Up => {
                    let index = match self.current_selected_sample.selected() {
                        Some(index) => {
                            let (index, overflow) = index.overflowing_sub(1);
                            if overflow {
                                None
                            } else {
                                Some(index)
                            }
                        }

                        None => None,
                    };

                    self.current_selected_sample.select(index);
                }

                _ => {}
            }
        }

        None
    }

    pub fn handle_update(&mut self, update: WatchUpdatesResponse) {
        if update.initial_state.is_some() {
            // Deal with initial_state
        } else {
            for network_update in update.network_update {
                if let Some(update) = network_update.update {
                    match update {
                        network_update::Update::ConnectedPeer(connected_peer) => {
                            self.network
                                .insert(connected_peer.peer_id.clone(), connected_peer);
                        }
                        network_update::Update::DisconnectedPeer(DisconnectedPeer { peer_id }) => {
                            self.network.remove(&peer_id);
                        }
                    }
                }
            }

            for double_echo_update in update.double_echo_update {
                if let Some(update) = double_echo_update.update {
                    match update {
                        double_echo_update::Update::NewSample(NewSample { sample }) => {
                            if let Some(sample) = sample {
                                let mut new_sample = HashMap::new();

                                new_sample
                                    .insert("EchoSubscribers".to_string(), sample.echo_subscribers);
                                new_sample.insert(
                                    "ReadySubscribers".to_string(),
                                    sample.ready_subscribers,
                                );
                                new_sample.insert(
                                    "EchoSubscriptions".to_string(),
                                    sample.echo_subsriptions,
                                );
                                new_sample.insert(
                                    "ReadySubscriptions".to_string(),
                                    sample.ready_subsriptions,
                                );
                                new_sample.insert(
                                    "DeliverySubscriptions".to_string(),
                                    sample.delivery_subsriptions,
                                );

                                self.samples = new_sample;
                            }
                        }
                    }
                }
            }

            for runtime_update in update.runtime_update {
                if let Some(update) = runtime_update.update {
                    match update {
                        runtime_update::Update::NewCertificate(cert) => {
                            self.certificates.insert(cert.cert_id.clone(), cert);
                        }
                    }
                }
            }
        }
    }
}
