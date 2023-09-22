use rstest::*;
use topos_core::uci::CertificateId;
use topos_crypto::messages::MessageSigner;
use topos_tce_transport::ValidatorId;

#[rstest]
pub fn test_signing_messages() {
    let message_signer_sender =
        MessageSigner::new("122f3ae6ade1fd136b292cea4f6243c7811160352c8821528547a1fe7c459daf");
    let validator_id_sender = ValidatorId::from(message_signer_sender.public_address);
    let certificate_id = CertificateId::from_array([0u8; 32]);

    let mut payload = Vec::new();
    payload.extend_from_slice(certificate_id.as_array());
    payload.extend_from_slice(validator_id_sender.as_bytes());

    let signature = message_signer_sender
        .sign_message(&payload)
        .expect("Cannot create Signature");

    let message_signer_receiver =
        MessageSigner::new("a2e33a9bad88f7b7568228f51d5274c471a9217162d46f1533b6a290f0be1baf");

    let verify = message_signer_receiver.verify_signature(
        signature,
        &payload,
        validator_id_sender.address(),
    );

    assert!(verify.is_ok());
}

#[rstest]
pub fn fails_to_verify_with_own_public_address() {
    let message_signer_sender =
        MessageSigner::new("122f3ae6ade1fd136b292cea4f6243c7811160352c8821528547a1fe7c459daf");
    let validator_id_sender = ValidatorId::from(message_signer_sender.public_address);
    let certificate_id = CertificateId::from_array([0u8; 32]);

    let mut payload = Vec::new();
    payload.extend_from_slice(certificate_id.as_array());
    payload.extend_from_slice(validator_id_sender.as_bytes());

    let signature = message_signer_sender
        .sign_message(&payload)
        .expect("Cannot create Signature");

    let message_signer_receiver =
        MessageSigner::new("a2e33a9bad88f7b7568228f51d5274c471a9217162d46f1533b6a290f0be1baf");
    let validator_id_receiver = ValidatorId::from(message_signer_receiver.public_address);

    let verify = message_signer_receiver.verify_signature(
        signature,
        &payload,
        validator_id_receiver.address(),
    );

    assert!(verify.is_err());
}
