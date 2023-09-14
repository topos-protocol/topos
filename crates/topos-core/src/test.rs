use crate::types::stream::Position;

#[test]
fn test_position() {
    let zero = Position::ZERO;

    let serialized = bincode::serialize(&zero).unwrap();

    let deserialized: Position = bincode::deserialize(&serialized).unwrap();

    assert_eq!(zero, deserialized);

    let one = Position(1);

    let serialized = bincode::serialize(&one).unwrap();

    let deserialized: Position = bincode::deserialize(&serialized).unwrap();

    assert_eq!(one, deserialized);
}
