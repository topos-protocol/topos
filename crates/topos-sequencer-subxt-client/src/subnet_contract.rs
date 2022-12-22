use crate::Error;
use ethabi::{ParamType, Token};
use ethereum_tx_sign::LegacyTransaction;
use secp256k1::{PublicKey, SecretKey};
use topos_sequencer_types::{Certificate, CrossChainTransaction, CrossChainTransactionData};

const ETH_CHAIN_ID: u64 = 43; // TODO: parametrize this in app config
const ETH_GAS_PRICE: u128 = 20 * 10u128.pow(9); // TODO: derive dynamically
const ETH_GAS_LIMIT: u128 = 10000000; // TODO: derive dynamically

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

pub fn subnet_encode_mint_call(
    cert: &Certificate,
    txs: &Vec<&CrossChainTransaction>,
) -> Result<Vec<u8>, crate::Error> {
    // Encode mint transaction call to topos core contract
    // Solidity function mint(bytes memory cert, CrossSubnetMessage memory _crossSubnetMessage) public onlyOwner */
    #[allow(deprecated)] // NOTE: To remove warning for deprecated constant field
    let tcc_mint_function = ethabi::Function {
        name: "mint".to_string(),
        inputs: vec![
            ethabi::Param {
                name: "cert".to_string(),
                kind: ParamType::Bytes,
                internal_type: None,
            },
            ethabi::Param {
                name: "crossSubnetMessage".to_string(),
                kind: ParamType::Tuple(vec![
                    ParamType::Uint(256), /* sendingSubnetId */
                    ParamType::Array(Box::new(
                        ParamType::Tuple(vec![
                            ParamType::Uint(256), /* recipientSubnetId */
                            ParamType::String,    /* assetId */
                            ParamType::Address,   /* recipientAddr */
                            ParamType::Uint(256), /* amount */
                        ]), /* CrossChainTransaction */
                    )), /* CrossChainTransaction[] inputs */
                    ParamType::Uint(256), /* isTypeOf */
                ]),
                internal_type: None,
            },
        ],
        outputs: Vec::new(),
        state_mutability: ethabi::StateMutability::NonPayable,
        constant: None,
    };

    // TODO: pack certificate relevant signature and check data to bytes
    let cert_bytes: ethabi::Bytes = Vec::new();

    // Prepare transaction data for smart contract mint call
    // Certificate
    let mut mint_transaction_data: Vec<Token> = vec![Token::Bytes(cert_bytes)];
    // CrossSubnetMessage
    let token_sending_subnet_id =
        Token::Uint(ethabi::Uint::from_str_radix(&cert.source_subnet_id, 10)?);
    let mut cross_chain_transaction_inputs: Vec<Token> = Vec::new();
    for tx in txs {
        if let CrossChainTransactionData::AssetTransfer { asset_id, amount } = &tx.transaction_data
        {
            let recipient_address: [u8; 20] = hex::decode(&tx.recipient_addr[2..])?
                .try_into()
                .map_err(|_| crate::Error::InvalidArgument {
                    message: "Invalid recipient address".to_string(),
                })?;
            cross_chain_transaction_inputs.push(Token::Tuple(vec![
                Token::Uint(ethabi::Uint::from_str_radix(&tx.terminal_subnet_id, 10)?),
                Token::String(asset_id.clone()),
                Token::Address(ethabi::Address::from(recipient_address)),
                Token::Uint(ethabi::Uint::from(amount)),
            ]));
        }
    }
    let token_inputs = Token::Array(cross_chain_transaction_inputs);
    let token_is_type_of = Token::Uint(ethabi::Uint::from(0) /* IsTypeOf::Inbount */);
    let token_cross_subnet_message = Token::Tuple(vec![
        token_sending_subnet_id,
        token_inputs,
        token_is_type_of,
    ]);
    mint_transaction_data.push(token_cross_subnet_message);
    // Encode mint function call with parameters to eth abi
    Ok(tcc_mint_function.encode_input(&mint_transaction_data[..])?)
}

pub fn subnet_prepare_transation(
    subnet_contract_address: &str,
    nonce: u128,
    transaction_data: &[u8],
) -> Result<LegacyTransaction, crate::Error> {
    let contract_address = hex::decode(&subnet_contract_address[2..])?;
    // TODO: switch to new transaction type
    // TODO: some parameters are hardcoded and should be parametrized (chain id, gas limit etc.)
    // Create Ethereum transaction
    Ok(LegacyTransaction {
        chain: ETH_CHAIN_ID,
        nonce,
        to: Some(
            contract_address
                .try_into()
                .map_err(|_| Error::InvalidArgument {
                    message: "invalid ethereum contract address".to_string(),
                })?,
        ),
        value: 0,
        gas_price: ETH_GAS_PRICE,
        gas: ETH_GAS_LIMIT,
        data: transaction_data.into(),
    })
}
