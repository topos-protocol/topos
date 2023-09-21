use ethers::prelude::{LocalWallet, Signer};
use rstest::*;
use topos_core::uci::CertificateId;
use topos_crypto::messages::{sign_message, verify_signature};
use topos_tce_transport::ValidatorId;

#[rstest]
pub fn test_signing_messages() {
    let wallet: LocalWallet = "47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6"
        .parse()
        .unwrap();
    let validator_id = ValidatorId::from(wallet.address());
    let certificate_id = CertificateId::from_array([0u8; 32]);

    let signature = sign_message(certificate_id.as_array(), validator_id.as_bytes(), &wallet)
        .expect("Cannot create Signature");

    let verify = verify_signature(
        signature,
        certificate_id.as_array(),
        validator_id.as_bytes(),
        validator_id.address(),
    );

    println!("verify: {:#?}", verify);

    assert!(verify.is_ok());
}
