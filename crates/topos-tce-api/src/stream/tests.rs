use rstest::*;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use topos_core::uci::SUBNET_ID_LENGTH;
use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::{SOURCE_SUBNET_ID_2, TARGET_SUBNET_ID_1};
use uuid::Uuid;

use self::utils::StreamBuilder;
use crate::grpc::messaging::{OutboundMessage, StreamOpened};
use crate::runtime::InternalRuntimeCommand;
use crate::stream::{StreamError, StreamErrorKind, TransientStream};
use crate::tests::encode;
use crate::wait_for_command;
use test_log::test;
use tokio::spawn;
use topos_core::api::grpc::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::api::grpc::shared::v1::positions::TargetStreamPosition;
use topos_core::api::grpc::tce::v1::watch_certificates_request::OpenStream as GrpcOpenStream;
use topos_core::api::grpc::tce::v1::WatchCertificatesRequest;

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
        "Doesn't match {result:?}",
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

    let expected_certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_2, &[TARGET_SUBNET_ID_1], 2);

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
        "Expected StreamOpened, received: {msg:?}"
    );

    for (index, expected_certificate) in expected_certificates.iter().enumerate() {
        context
            .command_sender
            .send(crate::stream::StreamCommand::PushCertificate {
                certificate: expected_certificate.clone(),
                positions: vec![topos_core::api::grpc::checkpoints::TargetStreamPosition {
                    position: index as u64,
                    certificate_id: Some(expected_certificate.certificate.id),
                    target_subnet_id: [1u8; SUBNET_ID_LENGTH].into(),
                    source_subnet_id: expected_certificate.certificate.source_subnet_id,
                }],
            })
            .await
            .expect("Unable to send certificate during test");
    }

    for (expected_position, expected_certificate) in expected_certificates.into_iter().enumerate() {
        assert!(
            matches!(
                context.stream_receiver.recv().await,
                Some(Ok((_, OutboundMessage::CertificatePushed(certificate_pushed)))) if certificate_pushed.certificate == expected_certificate
                && certificate_pushed.positions[0].position == expected_position as u64,
            ),
            "Expected CertificatePushed with {}, received: {:?}",
            expected_certificate.certificate.id,
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

#[test(tokio::test)]
async fn opening_transient_stream() {
    let (_sender, receiver) = mpsc::channel(1);
    let (notifier, check) = oneshot::channel();
    let id = Uuid::new_v4();

    let stream = TransientStream {
        inner: receiver,
        stream_id: id,
        notifier: Some(notifier),
    };

    tokio::spawn(async move {
        drop(stream);
    });

    let res = check.await;

    assert_eq!(res.unwrap(), id);
}

#[test(tokio::test)]
async fn opening_transient_stream_drop_sender() {
    let (sender, receiver) = mpsc::channel(1);
    let (notifier, check) = oneshot::channel();
    let id = Uuid::new_v4();

    let mut stream = TransientStream {
        inner: receiver,
        stream_id: id,
        notifier: Some(notifier),
    };

    let handle = tokio::spawn(async move { while stream.next().await.is_some() {} });

    tokio::time::sleep(Duration::from_millis(10)).await;
    drop(sender);

    let res = check.await;

    assert_eq!(res.unwrap(), id);
    assert!(handle.is_finished());
}
