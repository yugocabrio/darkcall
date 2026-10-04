// Feasibility gate for a shared row encoding. Exactly 224 bytes, standard SHA-256.
fn row_hash(input: [u8; 224]) -> [u32; 8] {
    let mut padded = [0; 256];
    for i in 0..224 { padded[i] = input[i]; }
    padded[224] = 128;
    // 224 bytes = 1792 bits, encoded as a 64-bit big-endian bit length.
    padded[254] = 7;
    let mut state = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    for block in 0..4 {
        let mut words = [0; 16];
        for i in 0..16 {
            for j in 0..4 { words[i] = words[i] * 256 + padded[block * 64 + i * 4 + j] as u32; }
        }
        state = std::hash::sha256_compression(words, state);
    }
    state
}

fn main(input: [u8; 224]) -> pub [u32; 8] { row_hash(input) }

#[test]
fn zero_row_vector() {
    // Independent Python hashlib.sha256(bytes(224)) vector, big-endian words.
    assert(row_hash([0; 224]) == [1857461798, 3727304429, 2760570749, 1290570042, 2697938760, 2790047484, 4262568492, 510135001]);
}
