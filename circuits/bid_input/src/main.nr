use darkcall::{context, bid_commitment, share_commitment};

fn validate(
    bid: u64, salt: Field, bid_shares: [Field; 3], salt_shares: [Field; 3],
    condition_digest: [u8; 32], id: [u8; 32], units: u32, reserve: u64,
    budget: u64, processing_fee: u64, commitment: Field, pair_commitments: [Field; 3],
) {
    assert(units > 0);
    assert(units <= 32);
    assert(reserve <= budget);
    assert(budget <= 2100000000000000);
    assert(processing_fee <= 2100000000000000 - budget);
    assert(bid <= budget);
    assert(bid as Field == bid_shares[0] + bid_shares[1] + bid_shares[2]);
    assert(salt == salt_shares[0] + salt_shares[1] + salt_shares[2]);
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    assert(bid_commitment(ctx, id, bid, salt) == commitment);
    for i in 0..3 {
        // TACEO layout: own additive share and previous party's additive share.
        assert(share_commitment(ctx, id, i, bid_shares[i], bid_shares[(i + 2) % 3], salt_shares[i], salt_shares[(i + 2) % 3]) == pair_commitments[i]);
    }
}

fn main(
    bid: u64, salt: Field, bid_shares: [Field; 3], salt_shares: [Field; 3],
    condition_digest: pub [u8; 32], id: pub [u8; 32], units: pub u32,
    reserve: pub u64, budget: pub u64, processing_fee: pub u64,
    commitment: pub Field, pair_commitments: pub [Field; 3],
) {
    validate(bid, salt, bid_shares, salt_shares, condition_digest, id, units, reserve, budget, processing_fee, commitment, pair_commitments);
}

fn fixture_pairs() -> [Field; 3] {
    let ctx = context([0; 32], 3, 20, 100, 15000);
    let a = [10, 20, 20];
    let s = [11, 12, 13];
    let mut result = [0; 3];
    for i in 0..3 { result[i] = share_commitment(ctx, [1; 32], i, a[i], a[(i + 2) % 3], s[i], s[(i + 2) % 3]); }
    result
}

#[test]
fn share_sum_and_pair_digests_valid() {
    let ctx = context([0; 32], 3, 20, 100, 15000);
    validate(50, 36, [10, 20, 20], [11, 12, 13], [0; 32], [1; 32], 3, 20, 100, 15000, bid_commitment(ctx, [1; 32], 50, 36), fixture_pairs());
}

#[test(should_fail)]
fn changed_share_rejected() {
    let ctx = context([0; 32], 3, 20, 100, 15000);
    validate(50, 36, [10, 20, 21], [11, 12, 13], [0; 32], [1; 32], 3, 20, 100, 15000, bid_commitment(ctx, [1; 32], 50, 36), fixture_pairs());
}

#[test(should_fail)]
fn wrong_pair_commitment_rejected() {
    let ctx = context([0; 32], 3, 20, 100, 15000);
    let mut pairs = fixture_pairs(); pairs[0] += 1;
    validate(50, 36, [10, 20, 20], [11, 12, 13], [0; 32], [1; 32], 3, 20, 100, 15000, bid_commitment(ctx, [1; 32], 50, 36), pairs);
}

#[test(should_fail)]
fn wrong_bid_opening_rejected() {
    let ctx = context([0; 32], 3, 20, 100, 15000);
    validate(51, 36, [10, 20, 21], [11, 12, 13], [0; 32], [1; 32], 3, 20, 100, 15000, bid_commitment(ctx, [1; 32], 50, 36), fixture_pairs());
}

