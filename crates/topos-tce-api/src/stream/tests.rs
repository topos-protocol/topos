use crate::runtime::InternalRuntimeCommand;
use crate::tests::encode;
use crate::wait_for_command;
use test_log::test;
use tokio::spawn;
use topos_core::api::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::api::shared::v1::SubnetId;
use topos_core::api::tce::v1::watch_certificates_request::OpenStream;
use topos_core::api::tce::v1::WatchCertificatesRequest;

use self::utils::StreamBuilder;

mod utils;

#[test(tokio::test)]
pub async fn sending_no_message() -> Result<(), Box<dyn std::error::Error>> {
    let (_, stream, mut context) = StreamBuilder::default().build();

    spawn(stream.run());

    wait_for_command!(
        context.stream_receiver,
        matches: Err(status) if status.message() == "No openstream provided"
    );

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::StreamTimeout { stream_id } if stream_id == expected_stream_id
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
                value: "stream_a".into(),
            }],
            positions: Vec::new(),
        }),
        source_checkpoint: None,
    }
    .into();
    {
        _ = tx.send_data(encode(&msg)?).await;
        let _ = tx;
    }

    let expected_stream_id = context.stream_id;

    wait_for_command!(
        context.runtime_receiver,
        matches: InternalRuntimeCommand::Register { stream_id, ref subnet_ids, .. } if stream_id == expected_stream_id && subnet_ids == &vec![SubnetId {value: "stream_a".into()}]
    );

    Ok(())
}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn subscribing_to_multiple_subnet_id() {}

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
