use std::{sync::Arc, time::Duration};

use crate::{
    graphql::query::{QueryRoot, SubscriptionRoot},
    runtime::InternalRuntimeCommand,
    stream::TransientStream,
};
use async_graphql::{http, value, EmptyMutation, Schema};
use futures::{SinkExt, StreamExt};
use rstest::rstest;
use test_log::test;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, INITIAL_CERTIFICATE_ID};
use topos_test_sdk::constants::{SOURCE_SUBNET_ID_2, TARGET_SUBNET_ID_3};
use uuid::Uuid;

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(2))]
async fn requesting_transient_stream_from_graphql() {
    let (sender, mut receiver) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut v = Vec::new();
        while let Some(query) = receiver.recv().await {
            if let InternalRuntimeCommand::NewTransientStream { sender } = query {
                let (notifier, notifier_receiver) = oneshot::channel();
                v.push(notifier_receiver);

                let (_s, inner) = mpsc::channel(10);
                _ = sender.send(Ok(TransientStream {
                    stream_id: Uuid::new_v4(),
                    notifier: Some(notifier),
                    inner,
                }));
            }
        }
    });

    let root = SubscriptionRoot {};

    let result = root.new_transient_stream(&sender, None).await;

    assert!(result.is_ok());
}

#[rstest]
#[timeout(Duration::from_secs(4))]
#[test(tokio::test)]
async fn open_watch_certificate_delivered() {
    let (mut tx, rx) = futures::channel::mpsc::unbounded();
    let (sender, mut receiver): (mpsc::Sender<InternalRuntimeCommand>, _) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut v = Vec::new();
        while let Some(query) = receiver.recv().await {
            if let InternalRuntimeCommand::NewTransientStream { sender } = query {
                let (notifier, notifier_receiver) = oneshot::channel();
                v.push(notifier_receiver);

                let (notify, inner) = mpsc::channel(10);
                _ = sender.send(Ok(TransientStream {
                    stream_id: Uuid::new_v4(),
                    notifier: Some(notifier),
                    inner,
                }));

                tokio::time::sleep(Duration::from_millis(10)).await;

                let certificate = Certificate::new_with_default_fields(
                    INITIAL_CERTIFICATE_ID,
                    SOURCE_SUBNET_ID_2,
                    &[TARGET_SUBNET_ID_3],
                )
                .unwrap();

                _ = notify.send(Arc::new(certificate)).await;
            }
        }
    });
    let subscription = SubscriptionRoot {};
    let schema = Schema::build(QueryRoot, EmptyMutation, subscription)
        .data(sender)
        .finish();

    let mut stream = http::WebSocket::new(schema, rx, http::WebSocketProtocols::GraphQLWS);

    tx.send(
        serde_json::to_string(&value!({
            "type": "connection_init",
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stream.next().await.unwrap().unwrap_text())
            .unwrap(),
        serde_json::json!({
            "type": "connection_ack",
        }),
    );

    tx.send(
        serde_json::to_string(&value!({
            "type": "start",
            "id": "1",
            "payload": {
                "query": "subscription onCertificates {
                              watchDeliveredCertificates {
                                id
                                prevId
                                proof
                                signature
                                sourceSubnetId { value }
                                stateRoot
                                targetSubnets {
                                  value
                                }
                                txRootHash
                                receiptsRootHash
                                verifier
                              }
                            }"
            },
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let certificate =
        &serde_json::from_str::<serde_json::Value>(&stream.next().await.unwrap().unwrap_text())
            .unwrap();

    let certificate = serde_json::from_value::<topos_api::graphql::certificate::Certificate>(
        certificate["payload"]["data"]["watchDeliveredCertificates"].clone(),
    )
    .unwrap();

    assert_eq!(certificate.source_subnet_id, SOURCE_SUBNET_ID_2,);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stream.next().await.unwrap().unwrap_text())
            .unwrap(),
        serde_json::json!({
            "type": "complete",
            "id": "1",
        }),
    );
}
