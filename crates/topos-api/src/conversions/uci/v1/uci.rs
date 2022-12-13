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
                    asset_id: asset_transfer.asset_id,
                    amount: U256::from_little_endian(&asset_transfer.amount[..]),
                }
            }
            proto_v1::cross_chain_transaction_data::Data::FunctionCall(function_call) => {
                topos_uci::CrossChainTransactionData::FunctionCall {
                    data: function_call.data,
                }
            }
        }
    }
}

impl From<topos_uci::CrossChainTransactionData> for proto_v1::CrossChainTransactionData {
    fn from(transaction_data: topos_uci::CrossChainTransactionData) -> Self {
        match transaction_data {
            topos_uci::CrossChainTransactionData::AssetTransfer { amount, asset_id } => {
                proto_v1::CrossChainTransactionData {
                    data: Some(proto_v1::cross_chain_transaction_data::Data::AssetTransfer(
                        proto_v1::cross_chain_transaction_data::AssetTransfer {
                            asset_id,
                            amount: {
                                let mut buffer: [u8; 32] = [0; 32];
                                amount.to_little_endian(&mut buffer);
                                buffer.to_vec()
                            },
                        },
                    )),
                }
            }
            topos_uci::CrossChainTransactionData::FunctionCall { data } => {
                proto_v1::CrossChainTransactionData {
                    data: Some(proto_v1::cross_chain_transaction_data::Data::FunctionCall(
                        proto_v1::cross_chain_transaction_data::FunctionCall { data },
                    )),
                }
            }
        }
    }
}

impl From<proto_v1::CrossChainTransaction> for topos_uci::CrossChainTransaction {
    fn from(transaction: proto_v1::CrossChainTransaction) -> Self {
        topos_uci::CrossChainTransaction {
            terminal_subnet_id: transaction.terminal_subnet_id,
            sender_addr: transaction.sender_addr,
            recipient_addr: transaction.recipient_addr,
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
            terminal_subnet_id: transaction.terminal_subnet_id,
            sender_addr: transaction.sender_addr,
            recipient_addr: transaction.recipient_addr,
            transaction_data: Some(transaction.transaction_data.into()),
        }
    }
}

impl From<proto_v1::Certificate> for topos_uci::Certificate {
    fn from(certificate: proto_v1::Certificate) -> Self {
        topos_uci::Certificate {
            initial_subnet_id: certificate.initial_subnet_id,
            cert_id: certificate.cert_id,
            prev_cert_id: certificate.prev_cert_id,
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
            initial_subnet_id: certificate.initial_subnet_id,
            cert_id: certificate.cert_id,
            prev_cert_id: certificate.prev_cert_id,
            calls: certificate
                .calls
                .into_iter()
                .map(|call| call.into())
                .collect(),
        }
    }
}
