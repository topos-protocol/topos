use crate::runtime::InternalRuntimeCommand;
use crate::stream::{StreamError, StreamErrorKind};
use crate::tests::encode;
use crate::wait_for_command;
use test_log::test;
use tokio::spawn;
use topos_core::api::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::api::shared::v1::positions::TargetStreamPosition;
use topos_core::api::shared::v1::SubnetId;
use topos_core::api::tce::v1::watch_certificates_request::OpenStream;
use topos_core::api::tce::v1::WatchCertificatesRequest;

use self::utils::StreamBuilder;

mod utils;

#[test(tokio::test)]
pub async fn sending_no_message() -> Result<(), Box<dyn std::error::Error>> {
    let (_, stream, mut context) = StreamBuilder::default().build();

    let join = spawn(stream.run());

    wait_for_command!(
        context.stream_receiver,
        matches: Err(status) if status.message() == "No OpenStream provided"
    );

    assert_eq!(
        Err(StreamError {
            stream_id: context.stream_id,
            kind: StreamErrorKind::PreStartError
        }),
        join.await?
    );

    Ok(())
}

#[test(tokio::test)]
pub async fn sending_open_stream_message() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = StreamBuilder::default().build();

    spawn(async move { stream.run().await });

    let msg: WatchCertificatesRequest = OpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            // FIX: subnet_id isn't following the right subnet format
            target_subnet_ids: vec![SubnetId {
                value: [1u8; 32].into(),
            }],
            positions: Vec::new(),
        }),
        source_checkpoint: None,
    }
    .into();

    _ = tx.send_data(encode(&msg)?).await;

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::Register { stream_id, ref subnet_ids, .. } if stream_id == expected_stream_id && subnet_ids == &vec![SubnetId {value: [1u8; 32].into()}]
    );

    Ok(())
}

#[test(tokio::test)]
async fn subscribing_to_one_target_with_position() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tx, stream, mut context) = StreamBuilder::default().build();

    spawn(async move { stream.run().await });

    let msg: WatchCertificatesRequest = OpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            // FIX: subnet_id isn't following the right subnet format
            target_subnet_ids: vec![SubnetId {
                value: [1u8; 32].into(),
            }],
            positions: vec![TargetStreamPosition {
                source_subnet_id: Some([2u8; 32].into()),
                target_subnet_id: Some([1u8; 32].into()),
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
        matches: InternalRuntimeCommand::Register { stream_id, ref subnet_ids, .. } if stream_id == expected_stream_id && subnet_ids == &vec![SubnetId {value: [1u8; 32].into()}]
    );

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
