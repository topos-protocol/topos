use crate::types::stream::Position;

#[test]
fn test_position() {
    let zero = Position::ZERO;

    let serialized = bincode::serialize(&zero).unwrap();

    let deserialized: Position = bincode::deserialize(&serialized).unwrap();

    assert_eq!(zero, deserialized);

    let one: u64 = 1;

    let serialized = bincode::serialize(&one).unwrap();

    let deserialized: Position = bincode::deserialize(&serialized).unwrap();

    assert_eq!(one, deserialized);
}

#[test]
fn position_from_integer() {
    let position: Position = (0u64).into();

    assert_eq!(*position, 0);
}
