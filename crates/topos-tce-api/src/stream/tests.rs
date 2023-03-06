use rstest::*;
use std::time::Duration;
use topos_core::uci::Certificate;
use topos_test_sdk::constants::{PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID_2, TARGET_SUBNET_ID_1};

use crate::grpc::messaging::{OutboundMessage, StreamOpened};
use crate::runtime::InternalRuntimeCommand;
use crate::stream::{StreamError, StreamErrorKind};
use crate::tests::encode;
use crate::wait_for_command;
use test_log::test;
use tokio::spawn;
use topos_core::api::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::api::shared::v1::positions::TargetStreamPosition;
use topos_core::api::tce::v1::watch_certificates_request::OpenStream as GrpcOpenStream;
use topos_core::api::tce::v1::WatchCertificatesRequest;

use self::utils::StreamBuilder;

mod utils;

#[rstest]
#[timeout(Duration::from_millis(100))]
#[test(tokio::test)]
pub async fn sending_no_message() -> Result<(), Box<dyn std::error::Error>> {
    let (_, stream, mut context) = StreamBuilder::default().build();

    let join = spawn(stream.run());

    wait_for_command!(
        context.stream_receiver,
        matches: Err(status) if status.message() == "No OpenStream provided"
    );

    let result = join.await?;

    assert!(
        matches!(result, Err(StreamError { stream_id, kind: StreamErrorKind::PreStartError}) if stream_id == context.stream_id),
        "Doesn't match {:?}",
        result
    );

    Ok(())
}

#[rstest]
#[timeout(Duration::from_millis(100))]
#[test(tokio::test)]
pub async fn sending_open_stream_message() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = StreamBuilder::default().build();

    let join = spawn(stream.run());

    let msg: WatchCertificatesRequest = GrpcOpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            target_subnet_ids: vec![TARGET_SUBNET_ID_1.into()],
            positions: Vec::new(),
        }),
        source_checkpoint: None,
    }
    .into();

    _ = tx.send_data(encode(&msg)?).await;

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::Register { stream_id, .. } if stream_id == expected_stream_id
    );

    join.abort();
    Ok(())
}

#[rstest]
#[timeout(Duration::from_millis(100))]
#[test(tokio::test)]
async fn subscribing_to_one_target_with_position() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = StreamBuilder::default().build();

    let join = spawn(stream.run());

    let msg: WatchCertificatesRequest = GrpcOpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            target_subnet_ids: vec![TARGET_SUBNET_ID_1.into()],
            positions: vec![TargetStreamPosition {
                source_subnet_id: Some(SOURCE_SUBNET_ID_2.into()),
                target_subnet_id: Some(TARGET_SUBNET_ID_1.into()),
                position: 1,
                certificate_id: None,
            }],
        }),
        source_checkpoint: None,
    }
    .into();

    _ = tx.send_data(encode(&msg)?).await;

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::Register { stream_id, .. } if stream_id == expected_stream_id
    );

    join.abort();

    Ok(())
}

#[rstest]
#[timeout(Duration::from_millis(100))]
#[test(tokio::test)]
async fn receive_expected_certificate_from_zero() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = StreamBuilder::default().build();

    let first = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_2,
        Default::default(),
        Default::default(),
        &vec![TARGET_SUBNET_ID_1],
        0,
        Default::default(),
    )
    .unwrap();
    let second = Certificate::new(
        first.id,
        SOURCE_SUBNET_ID_2,
        Default::default(),
        Default::default(),
        &vec![TARGET_SUBNET_ID_1],
        0,
        Default::default(),
    )
    .unwrap();

    let expected_certificates = vec![first, second];

    let join = spawn(stream.run());

    let msg: WatchCertificatesRequest = GrpcOpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            target_subnet_ids: vec![TARGET_SUBNET_ID_1.into()],
            positions: vec![],
        }),
        source_checkpoint: None,
    }
    .into();

    _ = tx.send_data(encode(&msg)?).await;

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::Register { stream_id, sender, .. } if stream_id == expected_stream_id => {
            sender.send(Ok(()))
        }
    );

    let msg = context.stream_receiver.recv().await;
    assert!(
        matches!(
            msg,
            Some(Ok((_, OutboundMessage::StreamOpened(StreamOpened { ref subnet_ids })))) if subnet_ids == &[TARGET_SUBNET_ID_1],
        ),
        "Expected StreamOpened, received: {:?}",
        msg
    );

    for expected_certificate in expected_certificates.iter() {
        context
            .command_sender
            .send(crate::stream::StreamCommand::PushCertificate {
                certificate: expected_certificate.clone(),
            })
            .await
            .expect("Unable to send certificate during test");
    }

    for expected_certificate in expected_certificates {
        assert!(
            matches!(
                context.stream_receiver.recv().await,
                Some(Ok((_, OutboundMessage::CertificatePushed(certificate_pushed)))) if certificate_pushed.certificate == expected_certificate,
            ),
            "Expected CertificatePushed with {}, received: {:?}",
            expected_certificate.id,
            msg
        );
    }

    join.abort();
    Ok(())
}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn pausing_all_subscription() {}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn pausing_one_subscription() {}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn resuming_one_subscription() {}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn resuming_all_subscription() {}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn closing_client_stream() {}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn closing_server_stream() {}
