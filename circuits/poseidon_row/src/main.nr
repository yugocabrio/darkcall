// Comparison only: exactly eight 30-byte BE chunks, final chunk 14 bytes.
// Existing BN254 Poseidon2 fixed sponge. Not yet native-output binding.
use darkcall::hash;
fn main(input: [u8; 224]) -> pub [u32; 8] {
    let mut packed = [0; 8];
    for i in 0..8 {
        for j in 0..30 {
            let index = i * 30 + j;
            if index < 224 { packed[i] = packed[i] * 256 + input[index] as Field; }
        }
    }
    let digest: [u8; 32] = hash(packed).to_be_bytes();
    let mut words = [0; 8];
    for i in 0..8 { for j in 0..4 { words[i] = words[i] * 256 + digest[i * 4 + j] as u32; } }
    words
}
