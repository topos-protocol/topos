///
/// Protobuf generated/native Rust structures related conversions for GRPC API
///
use crate::uci::v1 as proto_v1;
use ethereum_types::U256;

// TODO: replace From by TryFrom because From must not fail
impl From<proto_v1::CrossChainTransactionData> for topos_uci::CrossChainTransactionData {
    fn from(transaction_data: proto_v1::CrossChainTransactionData) -> Self {
        match transaction_data
            .data
            .expect("uci transaction data has content")
        {
            proto_v1::cross_chain_transaction_data::Data::AssetTransfer(asset_transfer) => {
                topos_uci::CrossChainTransactionData::AssetTransfer {
                    sender: asset_transfer
                        .sender
                        .try_into()
                        .expect("expected valid sender with correct length"),
                    receiver: asset_transfer
                        .receiver
                        .try_into()
                        .expect("expected valid sender with correct length"),
                    symbol: asset_transfer.symbol,
                    amount: U256::from_little_endian(&asset_transfer.amount[..]),
                }
            }
            proto_v1::cross_chain_transaction_data::Data::ContractCall(contract_call) => {
                topos_uci::CrossChainTransactionData::ContractCall {
                    source_contract_addr: contract_call
                        .source_contract_addr
                        .try_into()
                        .expect("expected valid source contract address with correct length"),
                    target_contract_addr: contract_call
                        .target_contract_addr
                        .try_into()
                        .expect("expected valid target contract address with correct length"),
                    payload: contract_call.payload,
                }
            }
            proto_v1::cross_chain_transaction_data::Data::ContractCallWithToken(
                contract_call_with_token,
            ) => topos_uci::CrossChainTransactionData::ContractCallWithToken {
                source_contract_addr: contract_call_with_token
                    .source_contract_addr
                    .try_into()
                    .expect("expected valid source contract address with correct length"),
                target_contract_addr: contract_call_with_token
                    .target_contract_addr
                    .try_into()
                    .expect("expected valid target contract address with correct length"),
                payload: contract_call_with_token.payload,
                symbol: contract_call_with_token.symbol,
                amount: U256::from_little_endian(&contract_call_with_token.amount[..]),
            },
        }
    }
}

impl From<topos_uci::CrossChainTransactionData> for proto_v1::CrossChainTransactionData {
    fn from(transaction_data: topos_uci::CrossChainTransactionData) -> Self {
        match transaction_data {
            topos_uci::CrossChainTransactionData::AssetTransfer {
                sender,
                receiver,
                symbol,
                amount,
            } => proto_v1::CrossChainTransactionData {
                data: Some(proto_v1::cross_chain_transaction_data::Data::AssetTransfer(
                    proto_v1::cross_chain_transaction_data::AssetTransfer {
                        sender: sender.as_slice().to_vec(),
                        receiver: receiver.as_slice().to_vec(),
                        symbol,
                        amount: {
                            let mut buffer: [u8; 32] = [0; 32];
                            amount.to_little_endian(&mut buffer);
                            buffer.to_vec()
                        },
                    },
                )),
            },
            topos_uci::CrossChainTransactionData::ContractCall {
                source_contract_addr,
                target_contract_addr,
                payload,
            } => proto_v1::CrossChainTransactionData {
                data: Some(proto_v1::cross_chain_transaction_data::Data::ContractCall(
                    proto_v1::cross_chain_transaction_data::ContractCall {
                        source_contract_addr: source_contract_addr.as_slice().to_vec(),
                        target_contract_addr: target_contract_addr.as_slice().to_vec(),
                        payload,
                    },
                )),
            },
            topos_uci::CrossChainTransactionData::ContractCallWithToken {
                source_contract_addr,
                target_contract_addr,
                payload,
                symbol,
                amount,
            } => proto_v1::CrossChainTransactionData {
                data: Some(
                    proto_v1::cross_chain_transaction_data::Data::ContractCallWithToken(
                        proto_v1::cross_chain_transaction_data::ContractCallWithToken {
                            source_contract_addr: source_contract_addr.as_slice().to_vec(),
                            target_contract_addr: target_contract_addr.as_slice().to_vec(),
                            payload,
                            symbol,
                            amount: {
                                let mut buffer: [u8; 32] = [0; 32];
                                amount.to_little_endian(&mut buffer);
                                buffer.to_vec()
                            },
                        },
                    ),
                ),
            },
        }
    }
}

impl From<proto_v1::CrossChainTransaction> for topos_uci::CrossChainTransaction {
    fn from(transaction: proto_v1::CrossChainTransaction) -> Self {
        topos_uci::CrossChainTransaction {
            target_subnet_id: transaction
                .target_subnet_id
                .expect("valid target subnet id")
                .value
                .try_into()
                .expect("expected valid target subnet id with correct length"),
            transaction_data: transaction
                .transaction_data
                .expect("valid transaction data")
                .into(),
        }
    }
}

impl From<topos_uci::CrossChainTransaction> for proto_v1::CrossChainTransaction {
    fn from(transaction: topos_uci::CrossChainTransaction) -> Self {
        proto_v1::CrossChainTransaction {
            target_subnet_id: Some(crate::shared::v1::SubnetId {
                value: transaction.target_subnet_id.to_vec(),
            }),
            transaction_data: Some(transaction.transaction_data.into()),
        }
    }
}

impl From<proto_v1::Certificate> for topos_uci::Certificate {
    fn from(certificate: proto_v1::Certificate) -> Self {
        topos_uci::Certificate {
            source_subnet_id: certificate
                .source_subnet_id
                .expect("valid source subnet id")
                .value
                .try_into()
                .expect("expected valid source subnet id with correct length"),
            id: certificate
                .id
                .expect("valid certificate id")
                .value
                .try_into()
                .expect("expected valid certificate id with correct length"),
            prev_id: certificate
                .prev_id
                .expect("valid previous certificate id")
                .value
                .try_into()
                .expect("expected valid previous certificate id with correct length"),
            calls: certificate
                .calls
                .into_iter()
                .map(|call| call.into())
                .collect(),
        }
    }
}

impl From<topos_uci::Certificate> for proto_v1::Certificate {
    fn from(certificate: topos_uci::Certificate) -> Self {
        proto_v1::Certificate {
            source_subnet_id: Some(crate::shared::v1::SubnetId {
                value: certificate.source_subnet_id.to_vec(),
            }),
            id: Some(crate::shared::v1::CertificateId {
                value: certificate.id.into(),
            }),
            prev_id: Some(crate::shared::v1::CertificateId {
                value: certificate.prev_id.into(),
            }),
            calls: certificate
                .calls
                .into_iter()
                .map(|call| call.into())
                .collect(),
        }
    }
}
