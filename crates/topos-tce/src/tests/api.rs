use std::sync::Arc;

use libp2p::PeerId;
use prost::Message;
use rstest::rstest;
use test_log::test;
use tokio::sync::{mpsc, oneshot};
use topos_core::api::grpc::tce::v1::{double_echo_request, DoubleEchoRequest, Echo, Gossip, Ready};
use topos_crypto::{messages::MessageSigner, validator_id::ValidatorId};
use topos_tce_storage::{store::WriteStore, types::PendingResult};
use topos_test_sdk::{
    certificates::create_certificate_chain,
    constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1},
};

use crate::AppContext;

use super::setup_test;

#[rstest]
#[test(tokio::test)]
async fn handle_new_certificate(
    #[future] setup_test: (
        AppContext,
        mpsc::Receiver<topos_p2p::Command>,
        Arc<MessageSigner>,
    ),
) {
    let (mut context, mut p2p_receiver, message_signer) = setup_test.await;
    let mut certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 1);
    let certificate = certificates.pop().unwrap().certificate;

    let (sender, receiver) = oneshot::channel();

    context
        .on_api_event(topos_tce_api::RuntimeEvent::CertificateSubmitted {
            certificate: Box::new(certificate),
            sender,
        })
        .await;

    let response = receiver.await;

    assert!(matches!(response, Ok(Ok(PendingResult::InPending(_)))));
}

#[rstest]
#[test(tokio::test)]
async fn handle_certificate_in_precedence_pool(
    #[future] setup_test: (
        AppContext,
        mpsc::Receiver<topos_p2p::Command>,
        Arc<MessageSigner>,
    ),
) {
    let (mut context, mut p2p_receiver, message_signer) = setup_test.await;
    let mut certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 2);
    let certificate = certificates.pop().unwrap().certificate;

    let (sender, receiver) = oneshot::channel();

    context
        .on_api_event(topos_tce_api::RuntimeEvent::CertificateSubmitted {
            certificate: Box::new(certificate),
            sender,
        })
        .await;

    let response = receiver.await;

    assert!(matches!(response, Ok(Ok(PendingResult::AwaitPrecedence))));
}

#[rstest]
#[test(tokio::test)]
async fn handle_certificate_already_delivered(
    #[future] setup_test: (
        AppContext,
        mpsc::Receiver<topos_p2p::Command>,
        Arc<MessageSigner>,
    ),
) {
    let (mut context, mut p2p_receiver, message_signer) = setup_test.await;
    let mut certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 1);
    let certificate_delivered = certificates.pop().unwrap();

    _ = context
        .validator_store
        .insert_certificate_delivered(&certificate_delivered)
        .await
        .unwrap();

    let certificate = certificate_delivered.certificate;

    let (sender, receiver) = oneshot::channel();

    context
        .on_api_event(topos_tce_api::RuntimeEvent::CertificateSubmitted {
            certificate: Box::new(certificate),
            sender,
        })
        .await;

    let response = receiver.await;

    assert!(matches!(response, Ok(Ok(PendingResult::AlreadyDelivered))));
}
