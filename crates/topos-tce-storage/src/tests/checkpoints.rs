use std::{collections::HashMap, sync::Arc};

use rstest::rstest;
use topos_core::uci::SubnetId;
use topos_test_sdk::{
    certificates::create_certificate_chain,
    constants::{SOURCE_SUBNET_ID_1, SOURCE_SUBNET_ID_2, TARGET_SUBNET_ID_1},
};

use super::support::store;
use crate::{
    store::{ReadStore, WriteStore},
    validator::ValidatorStore,
};

#[rstest]
#[tokio::test]
async fn get_checkpoint_for_two_subnets(store: Arc<ValidatorStore>) {
    let certificates_a = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 32);
    let certificates_b = create_certificate_chain(SOURCE_SUBNET_ID_2, &[TARGET_SUBNET_ID_1], 24);

    for cert in certificates_a {
        _ = store.insert_certificate_delivered(&cert).await;
    }

    for cert in certificates_b {
        _ = store.insert_certificate_delivered(&cert).await;
    }

    let checkpoint = store
        .get_checkpoint()
        .unwrap()
        .into_iter()
        .map(|(subnet, value)| (subnet, *value.position))
        .collect::<HashMap<SubnetId, u64>>();

    assert_eq!(checkpoint.len(), 2);
    assert_eq!(*checkpoint.get(&SOURCE_SUBNET_ID_1).unwrap(), 31);
    assert_eq!(*checkpoint.get(&SOURCE_SUBNET_ID_2).unwrap(), 23);
}

#[rstest]
#[tokio::test]
async fn get_checkpoint_diff_with_no_input(store: Arc<ValidatorStore>) {
    let certificates_a = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 32);
    let certificates_b = create_certificate_chain(SOURCE_SUBNET_ID_2, &[TARGET_SUBNET_ID_1], 24);

    for cert in certificates_a {
        _ = store.insert_certificate_delivered(&cert).await;
    }

    for cert in certificates_b {
        _ = store.insert_certificate_delivered(&cert).await;
    }

    let checkpoint = store
        .get_checkpoint_diff(vec![])
        .unwrap()
        .into_iter()
        .map(|(subnet, proofs)| {
            (
                subnet,
                proofs
                    .iter()
                    .map(|proof| *proof.delivery_position.position)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<SubnetId, _>>();

    assert_eq!(checkpoint.len(), 2);
    assert_eq!(
        *checkpoint.get(&SOURCE_SUBNET_ID_1).unwrap(),
        (0..=31).collect::<Vec<_>>()
    );
    assert_eq!(
        *checkpoint.get(&SOURCE_SUBNET_ID_2).unwrap(),
        (0..=23).collect::<Vec<_>>()
    );
}

#[rstest]
#[tokio::test]
async fn get_checkpoint_diff_with_input(store: Arc<ValidatorStore>) {
    let certificates_a = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 32);
    let certificates_b = create_certificate_chain(SOURCE_SUBNET_ID_2, &[TARGET_SUBNET_ID_1], 24);

    let checkpoint = certificates_a.get(20).unwrap().proof_of_delivery.clone();
    assert_eq!(*checkpoint.delivery_position.position, 20);

    for cert in certificates_a {
        _ = store.insert_certificate_delivered(&cert).await;
    }

    for cert in certificates_b {
        _ = store.insert_certificate_delivered(&cert).await;
    }

    let checkpoint = store
        .get_checkpoint_diff(vec![checkpoint])
        .unwrap()
        .into_iter()
        .map(|(subnet, proofs)| {
            (
                subnet,
                proofs
                    .iter()
                    .map(|proof| *proof.delivery_position.position)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<SubnetId, _>>();

    assert_eq!(checkpoint.len(), 2);
    assert_eq!(
        *checkpoint.get(&SOURCE_SUBNET_ID_1).unwrap(),
        (21..=31).collect::<Vec<_>>()
    );
    assert_eq!(
        *checkpoint.get(&SOURCE_SUBNET_ID_2).unwrap(),
        (0..=23).collect::<Vec<_>>()
    );
}
