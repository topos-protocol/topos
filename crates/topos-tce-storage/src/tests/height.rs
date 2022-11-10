use crate::Height;

#[test]
fn test_height() {
    let zero = Height::ZERO;

    let serialized = bincode::serialize(&zero).unwrap();

    let deserialized: Height = bincode::deserialize(&serialized).unwrap();

    assert_eq!(zero, deserialized);

    let one = Height(1);

    let serialized = bincode::serialize(&one).unwrap();

    let deserialized: Height = bincode::deserialize(&serialized).unwrap();

    assert_eq!(one, deserialized);
}

#[tokio::test]
#[ignore = "not yet implemented"]
async fn height_can_be_fetch_for_multiple_subnets() {}

#[tokio::test]
#[ignore = "not yet implemented"]
async fn height_can_be_fetch_for_all_subnets() {}
