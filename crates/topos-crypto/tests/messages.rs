use rstest::*;
use topos_core::uci::CertificateId;
use topos_crypto::messages::MessageSigner;
use topos_tce_transport::ValidatorId;

#[rstest]
pub fn test_signing_messages() {
    let message_signer =
        MessageSigner::new("47d361f6becb933a77d7e01dee7b1c1859b656adbd8428bf7bf9519503e5d5d6");
    let validator_id = ValidatorId::from(message_signer.public_address);
    let certificate_id = CertificateId::from_array([0u8; 32]);

    let signature = message_signer
        .sign_message(certificate_id.as_array(), validator_id.as_bytes())
        .expect("Cannot create Signature");

    let verify = message_signer.verify_signature(
        signature,
        certificate_id.as_array(),
        validator_id.as_bytes(),
    );

    println!("verify: {:#?}", verify);

    assert!(verify.is_ok());
}
