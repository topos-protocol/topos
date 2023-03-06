use keccak_hash::keccak_256;

pub fn calculate_hash(data: &[u8]) -> [u8; 32] {
    let mut hash: [u8; 32] = [0u8; 32];
    keccak_256(data, &mut hash);
    hash
}
