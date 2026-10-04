// Shared 224-byte SHA-256 row encoding. Recipient points are canonical compressed
// g_d and pk_d. Native binding checks their curve and byte representation.
pub struct Row {
    pub conditions: [u8; 32],
    pub accepted_root: [u8; 32],
    pub ticket: [u8; 32],
    pub role: u32,
    pub action_index: u32,
    pub gd: [u8; 32],
    pub pkd: [u8; 32],
    pub amount: u64,
    pub salt: [u8; 32],
}

pub fn encode(row: Row) -> [u8; 224] {
    let domain = [68, 97, 114, 107, 67, 97, 108, 108, 47, 114, 111, 119, 47, 118, 49, 0];
    let mut result = [0; 224];
    for i in 0..16 { result[i] = domain[i]; }
    for i in 0..32 {
        result[16 + i] = row.conditions[i];
        result[48 + i] = row.accepted_root[i];
        result[80 + i] = row.ticket[i];
        result[120 + i] = row.gd[i];
        result[152 + i] = row.pkd[i];
        result[192 + i] = row.salt[i];
    }
    for i in 0..4 {
        result[112 + i] = ((row.role >> (i * 8)) & 255) as u8;
        result[116 + i] = ((row.action_index >> (i * 8)) & 255) as u8;
    }
    for i in 0..8 { result[184 + i] = ((row.amount >> ((i as u64) * 8)) & 255) as u8; }
    result
}

pub fn sha256_224(input: [u8; 224]) -> [u32; 8] {
    let mut padded = [0; 256];
    for i in 0..224 { padded[i] = input[i]; }
    padded[224] = 128;
    padded[254] = 7;
    let mut state = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for block in 0..4 {
        let mut words = [0; 16];
        for i in 0..16 { for j in 0..4 { words[i] = words[i] * 256 + padded[block * 64 + i * 4 + j] as u32; } }
        state = std::hash::sha256_compression(words, state);
    }
    state
}

pub fn commitment(row: Row) -> [u32; 8] { sha256_224(encode(row)) }

#[test]
fn encoding_matches_independent_python_vector() {
    let mut root = [0; 32]; root[31] = 2;
    let row = Row { conditions: [1; 32], accepted_root: root, ticket: [3; 32], role: 0, action_index: 5, gd: [4; 32], pkd: [5; 32], amount: 50, salt: [6; 32] };
    assert(commitment(row) == [2976452346, 2644444602, 4093154051, 1752889704, 1917315153, 1897124546, 1306150620, 994172045]);
    let encoded = encode(row);
    assert(encoded[79] == 2);
    assert(encoded[116] == 5);
    assert(encoded[117] == 0);
    assert(encoded[184] == 50);
    assert(encoded[185] == 0);
}

#[test]
fn rows_bind_action_amount_and_recipient() {
    let row = Row { conditions: [1; 32], accepted_root: [2; 32], ticket: [3; 32], role: 0, action_index: 5, gd: [4; 32], pkd: [5; 32], amount: 50, salt: [6; 32] };
    let original = commitment(row);
    let mut changed = row; changed.action_index = 4;
    assert(original != commitment(changed));
    changed = row; changed.amount = 51;
    assert(original != commitment(changed));
    changed = row; changed.pkd[31] = 4;
    assert(original != commitment(changed));
}
