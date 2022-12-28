use crate::Error;
use log::debug;
use secp256k1::{PublicKey, SecretKey};
use topos_sequencer_types::SubnetEvent;
use web3::ethabi;
use web3::ethabi::ParamType;
use web3::transports::WebSocket;
use web3::types::H160;

pub(crate) fn create_topos_core_contract_from_json(
    web3: &web3::Web3<WebSocket>,
    contract_address: &str,
) -> Result<web3::contract::Contract<WebSocket>, Error> {
    debug!("Creating topos core contract...");
    let contract_address_h160: H160 = H160::from_slice(&hex::decode(&contract_address[2..42])?);
    let contract = web3::contract::Contract::from_json(
        web3.eth(),
        contract_address_h160,
        include_bytes!("../abi/ToposCoreContract.json"),
    )
    .map_err(|e| Error::EthAbiError { source: e })?;
    Ok(contract)
}

pub(crate) fn parse_events_from_json() -> Result<Vec<web3::ethabi::Event>, Error> {
    let mut result = Vec::new();
    let contract_bytes = include_bytes!("../abi/ToposCoreContract.json");
    let reader = std::io::Cursor::new(contract_bytes);
    let contract = web3::ethabi::Contract::load(reader)?;
    for event in contract.events() {
        result.push(event.clone());
    }

    Ok(result)
}

fn get_event_type_from_log<'a, 'b>(
    events: &'a [web3::ethabi::Event],
    log: &'b web3::types::Log,
) -> Option<&'a web3::ethabi::Event> {
    events
        .iter()
        .find(|&event| event.signature() == log.topics[0])
}

pub(crate) fn parse_events_from_log(
    events: &[web3::ethabi::Event],
    logs: Vec<web3::types::Log>,
) -> Result<Vec<SubnetEvent>, Error> {
    let mut result = Vec::new();
    println!("Logs: {:?}", logs);
    for log in &logs {
        if let Some(event) = get_event_type_from_log(events, log) {
            match event.name.as_str() {
                "TokenSent" => {
                    // Parse TokenSent event
                    let sender = ethabi::decode(
                        vec![event.inputs[0].kind.clone()].as_slice(),
                        &log.topics[1].0,
                    )?;
                    let event_arguments = ethabi::decode(
                        &event
                            .inputs
                            .iter()
                            .skip(1)
                            .map(|i| i.kind.clone())
                            .collect::<Vec<ParamType>>(),
                        &log.data.0,
                    )?;
                    let send_token_event = SubnetEvent::TokenSent {
                        sender: if let ethabi::Token::Address(address) = sender[0] {
                            address.as_bytes().to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid sender address".to_string(),
                            });
                        },
                        source_subnet_id: if let ethabi::Token::FixedBytes(bytes) =
                            &event_arguments[0]
                        {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid source subnet id".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid source subnet id".to_string(),
                            });
                        },
                        target_subnet_id: if let ethabi::Token::FixedBytes(bytes) =
                            &event_arguments[1]
                        {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid target subnet id".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid target subnet id".to_string(),
                            });
                        },
                        receiver: if let ethabi::Token::Address(address) = event_arguments[2] {
                            address.as_bytes().to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid receiver address".to_string(),
                            });
                        },
                        symbol: if let ethabi::Token::String(symbol) = &event_arguments[3] {
                            symbol.clone()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid symbol".to_string(),
                            });
                        },
                        amount: if let ethabi::Token::Uint(value) = event_arguments[4] {
                            value
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid amount event argument".to_string(),
                            });
                        },
                    };
                    result.push(send_token_event);
                }
                "ContractCall" => {
                    // Parse ContractCall event
                    let payload_hash = ethabi::decode(
                        vec![event.inputs[0].kind.clone()].as_slice(),
                        &log.topics[1].0,
                    )?;
                    let event_arguments = ethabi::decode(
                        &event
                            .inputs
                            .iter()
                            .filter(|ep| ep.name != "payloadHash")
                            .map(|i| i.kind.clone())
                            .collect::<Vec<ParamType>>(),
                        &log.data.0,
                    )?;
                    let contact_call = SubnetEvent::ContractCall {
                        source_subnet_id: if let ethabi::Token::FixedBytes(bytes) =
                            &event_arguments[0]
                        {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid source subnet id".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid source subnet id".to_string(),
                            });
                        },
                        source_contract_addr: if let ethabi::Token::Address(address) =
                            event_arguments[1]
                        {
                            address.as_bytes().to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid source contract address".to_string(),
                            });
                        },
                        target_subnet_id: if let ethabi::Token::FixedBytes(bytes) =
                            &event_arguments[2]
                        {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid target subnet id".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid target subnet id".to_string(),
                            });
                        },
                        target_contract_addr: if let ethabi::Token::Address(address) =
                            event_arguments[3]
                        {
                            address.as_bytes().to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid target contract address".to_string(),
                            });
                        },
                        payload_hash: if let ethabi::Token::FixedBytes(bytes) = &payload_hash[0] {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid payload hash".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid payload hash".to_string(),
                            });
                        },
                        payload: if let ethabi::Token::Bytes(bytes) = &event_arguments[4] {
                            bytes.to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid payload".to_string(),
                            });
                        },
                    };

                    result.push(contact_call);
                }
                "ContractCallWithToken" => {
                    // Parse ContractCallWithToken event
                    let payload_hash = ethabi::decode(
                        vec![event.inputs[0].kind.clone()].as_slice(),
                        &log.topics[1].0,
                    )?;
                    let event_arguments = ethabi::decode(
                        &event
                            .inputs
                            .iter()
                            .filter(|ep| ep.name != "payloadHash")
                            .map(|i| i.kind.clone())
                            .collect::<Vec<ParamType>>(),
                        &log.data.0,
                    )?;
                    let contact_call = SubnetEvent::ContractCallWithToken {
                        source_subnet_id: if let ethabi::Token::FixedBytes(bytes) =
                            &event_arguments[0]
                        {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid source subnet id".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid source subnet id".to_string(),
                            });
                        },
                        source_contract_addr: if let ethabi::Token::Address(address) =
                            event_arguments[1]
                        {
                            address.as_bytes().to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid source contract address".to_string(),
                            });
                        },
                        target_subnet_id: if let ethabi::Token::FixedBytes(bytes) =
                            &event_arguments[2]
                        {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid target subnet id".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid target subnet id".to_string(),
                            });
                        },
                        target_contract_addr: if let ethabi::Token::Address(address) =
                            event_arguments[3]
                        {
                            address.as_bytes().to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid target contract address".to_string(),
                            });
                        },
                        payload_hash: if let ethabi::Token::FixedBytes(bytes) = &payload_hash[0] {
                            match bytes.clone().try_into() {
                                Ok(sender) => sender,
                                Err(_) => {
                                    return Err(Error::InvalidArgument {
                                        message: "invalid payload hash".to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid payload hash".to_string(),
                            });
                        },
                        payload: if let ethabi::Token::Bytes(bytes) = &event_arguments[4] {
                            bytes.to_vec()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid payload".to_string(),
                            });
                        },
                        symbol: if let ethabi::Token::String(symbol) = &event_arguments[5] {
                            symbol.clone()
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid symbol".to_string(),
                            });
                        },
                        amount: if let ethabi::Token::Uint(value) = event_arguments[6] {
                            value
                        } else {
                            return Err(Error::InvalidArgument {
                                message: "invalid amount event argument".to_string(),
                            });
                        },
                    };
                    result.push(contact_call);
                }
                _ => {
                    // Event not recognised, ignore it
                }
            }
        } else {
            // Event not recognised, ignore it
            continue;
        }
    }

    Ok(result)
}

pub fn derive_eth_address(secret_key: &[u8]) -> Result<String, crate::Error> {
    let eth_public_key: Vec<u8> = PublicKey::from_secret_key(
        &secp256k1::Secp256k1::new(),
        &SecretKey::from_slice(secret_key)?,
    )
    .serialize_uncompressed()[1..]
        .to_vec();
    let keccak = tiny_keccak::keccak256(&eth_public_key);

    Ok("0x".to_string() + &hex::encode(&keccak[..])[24..])
}
