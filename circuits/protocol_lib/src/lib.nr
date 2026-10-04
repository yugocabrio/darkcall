// Shared auction rules and Poseidon2 commitment encoding.
global N: u32 = 4;
global MAX_MONEY: u64 = 2100000000000000;

pub struct Result {
    pub price: u64,
    pub sold: u32,
    pub seller_amount: u64,
    pub operator_amount: u64,
    pub network_fee: u64,
    pub action_count: u32,
    pub refunds: [u64; N],
}

pub fn id_less(a: [u8; 32], b: [u8; 32]) -> bool {
    let mut less = false;
    let mut equal = true;
    for i in 0..32 {
        less = less | (equal & (a[i] < b[i]));
        equal = equal & (a[i] == b[i]);
    }
    less
}

pub fn auction(
    bids: [u64; N],
    ids: [[u8; 32]; N],
    active: [bool; N],
    units: u32,
    reserve: u64,
    budget: u64,
    processing_fee: u64,
) -> Result {
    assert(units > 0);
    assert(units <= 32);
    assert(reserve <= budget);
    assert(budget <= MAX_MONEY);
    assert(processing_fee <= MAX_MONEY - budget);
    let mut n = 0;
    for i in 0..N {
        if active[i] {
            assert(bids[i] <= budget);
            n = n + 1;
            for j in 0..N {
                if active[j] & (i != j) {
                    assert(ids[i] != ids[j]);
                }
            }
        } else {
            assert(bids[i] == 0);
        }
    }
    let total_input = (n as u64) * (budget + processing_fee);
    assert(total_input <= MAX_MONEY);
    let action_count = if n == 0 { 0 } else { n + 2 };
    let network_fee = (action_count as u64) * 5000;
    assert((n as u64) * processing_fee >= network_fee);
    let operator_amount = (n as u64) * processing_fee - network_fee;
    let mut price = reserve;
    let mut sold = 0;
    let mut refunds = [0; N];
    for i in 0..N {
        let qualified = active[i] & (bids[i] >= reserve);
        let mut rank = 0;
        for j in 0..N {
            let ahead = (bids[j] > bids[i]) | ((bids[j] == bids[i]) & id_less(ids[j], ids[i]));
            if active[j] & (bids[j] >= reserve) & ahead {
                rank = rank + 1;
            }
        }
        if qualified & (rank == units) {
            price = bids[i];
        }
        if qualified & (rank < units) {
            sold = sold + 1;
        }
    }
    let mut refund_total = 0;
    for i in 0..N {
        let qualified = active[i] & (bids[i] >= reserve);
        let mut rank = 0;
        for j in 0..N {
            let ahead = (bids[j] > bids[i]) | ((bids[j] == bids[i]) & id_less(ids[j], ids[i]));
            if active[j] & (bids[j] >= reserve) & ahead {
                rank = rank + 1;
            }
        }
        let won = qualified & (rank < units);
        refunds[i] = if active[i] {
            budget - if won { price } else { 0 }
        } else {
            0
        };
        refund_total = refund_total + refunds[i];
    }
    let seller_amount = (sold as u64) * price;
    assert(refund_total + seller_amount + operator_amount + network_fee == total_input);
    Result { price, sold, seller_amount, operator_amount, network_fee, action_count, refunds }
}


global D_CONTEXT: Field = 0x4461726b43616c6c2f6374782f7631;
global D_BID: Field = 0x4461726b43616c6c2f6269642f7631;
global D_ACCEPTED: Field = 0x4461726b43616c6c2f61636365707465642f7631;
global D_SHARE: Field = 0x4461726b43616c6c2f73686172652f7631;

// Noir rc.2's fixed-length Poseidon2 sponge: rate 3, capacity 1, length IV.
// Do not replace this with a single untagged permutation.
pub fn hash<let L: u32>(input: [Field; L]) -> Field {
    let mut state = [0; 4];
    state[3] = (L as Field) * 18446744073709551616;
    for i in 0..L / 3 {
        for j in 0..3 { state[j] += input[i * 3 + j]; }
        state = std::hash::poseidon2_permutation(state);
    }
    for i in 0..L % 3 { state[i] += input[(L / 3) * 3 + i]; }
    std::hash::poseidon2_permutation(state)[0]
}

pub fn limbs(bytes: [u8; 32]) -> [Field; 4] {
    let mut result = [0; 4];
    for i in 0..4 {
        let mut value: u64 = 0;
        for j in 0..8 { value = value * 256 + bytes[i * 8 + j] as u64; }
        result[i] = value as Field;
    }
    result
}

pub fn context(digest: [u8; 32], units: u32, reserve: u64, budget: u64, fee: u64) -> Field {
    let x = limbs(digest);
    hash([D_CONTEXT, x[0], x[1], x[2], x[3], units as Field, reserve as Field, budget as Field, fee as Field])
}

pub fn bid_commitment(ctx: Field, id: [u8; 32], bid: u64, salt: Field) -> Field {
    let x = limbs(id);
    hash([D_BID, ctx, x[0], x[1], x[2], x[3], bid as Field, salt])
}

pub fn share_commitment(ctx: Field, id: [u8; 32], party: u32, bid_a: Field, bid_b: Field, salt_a: Field, salt_b: Field) -> Field {
    let x = limbs(id);
    hash([D_SHARE, ctx, x[0], x[1], x[2], x[3], party as Field, bid_a, bid_b, salt_a, salt_b])
}

pub fn accepted_root(ctx: Field, active: [bool; N], commitments: [Field; N]) -> Field {
    let mut n = 0;
    for i in 0..N {
        if active[i] { n += 1; } else { assert(commitments[i] == 0); }
    }
    hash([D_ACCEPTED, ctx, N as Field, n as Field, active[0] as Field, active[1] as Field, active[2] as Field, active[3] as Field, commitments[0], commitments[1], commitments[2], commitments[3]])
}

pub fn verify_accepted(bids: [u64; N], salts: [Field; N], ids: [[u8; 32]; N], active: [bool; N], ctx: Field, commitments: [Field; N], root: Field) {
    for i in 0..N {
        if active[i] {
            assert(bid_commitment(ctx, ids[i], bids[i], salts[i]) == commitments[i]);
            if i > 0 {
                assert(active[i - 1]);
                assert(id_less(ids[i - 1], ids[i]));
            }
        } else {
            assert(ids[i] == [0; 32]);
            assert(bids[i] == 0);
            assert(salts[i] == 0);
        }
    }
    assert(accepted_root(ctx, active, commitments) == root);
}

#[test]
fn fixed_hash_matches_noir_rc2_vector() {
    assert(hash([1, 2, 3, 4, 5]) == 0x2247be7014a54d17342a7ef677f58d28877780d203860396967f5d0a18d259db);
}

#[test]
fn digest_encoding_keeps_all_bytes() {
    let mut a = [0; 32];
    let mut b = a;
    a[0] = 1;
    b[31] = 1;
    assert(limbs(a) != limbs(b));
    assert(limbs(a)[0] == 72057594037927936);
    assert(limbs(b)[3] == 1);
}

#[test]
fn changed_opening_or_context_changes_commitment() {
    let id = [7; 32];
    assert(bid_commitment(1, id, 50, 123) != bid_commitment(1, id, 51, 123));
    assert(bid_commitment(1, id, 50, 123) != bid_commitment(2, id, 50, 123));
    assert(bid_commitment(1, id, 50, 123) != bid_commitment(1, id, 50, 124));
}

fn accepted_fixture() -> ([[u8; 32]; 4], [Field; 4], Field) {
    let ids = [[1; 32], [2; 32], [3; 32], [4; 32]];
    let mut leaves = [0; 4];
    for i in 0..4 { leaves[i] = bid_commitment(7, ids[i], 50, i as Field + 100); }
    (ids, leaves, accepted_root(7, [true; 4], leaves))
}

#[test]
fn accepted_book_is_bound_to_openings() {
    let (ids, leaves, root) = accepted_fixture();
    verify_accepted([50; 4], [100, 101, 102, 103], ids, [true; 4], 7, leaves, root);
}

#[test(should_fail)]
fn removed_bid_cannot_reuse_accepted_root() {
    let (mut ids, mut leaves, root) = accepted_fixture();
    ids[3] = [0; 32]; leaves[3] = 0;
    verify_accepted([50, 50, 50, 0], [100, 101, 102, 0], ids, [true, true, true, false], 7, leaves, root);
}

#[test(should_fail)]
fn changed_bid_cannot_reuse_opening() {
    let (ids, leaves, root) = accepted_fixture();
    verify_accepted([51, 50, 50, 50], [100, 101, 102, 103], ids, [true; 4], 7, leaves, root);
}

#[test(should_fail)]
fn book_cannot_replay_under_different_conditions() {
    let (ids, leaves, root) = accepted_fixture();
    verify_accepted([50; 4], [100, 101, 102, 103], ids, [true; 4], 8, leaves, root);
}

#[test(should_fail)]
fn accepted_slots_must_be_a_sorted_prefix() {
    let (mut ids, mut leaves, _) = accepted_fixture();
    ids[1] = [0; 32]; leaves[1] = 0;
    let active = [true, false, true, true];
    let root = accepted_root(7, active, leaves);
    verify_accepted([50, 0, 50, 50], [100, 0, 102, 103], ids, active, 7, leaves, root);
}
