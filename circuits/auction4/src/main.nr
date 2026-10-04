// Committed-input computation gate; native settlement binding remains unfinished.
use darkcall::{auction, context, verify_accepted};
global N: u32 = 4;

fn main(
    bid0: u64,
    bid1: u64,
    bid2: u64,
    bid3: u64,
    salt0: Field,
    salt1: Field,
    salt2: Field,
    salt3: Field,
    condition_digest: pub [u8; 32],
    ids: pub [[u8; 32]; N],
    active: pub [bool; N],
    units: pub u32,
    reserve: pub u64,
    budget: pub u64,
    processing_fee: pub u64,
    commitments: pub [Field; N],
    root: pub Field,
) -> pub (u64, u32) {
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    verify_accepted([bid0, bid1, bid2, bid3], [salt0, salt1, salt2, salt3], ids, active, ctx, commitments, root);
    let result = auction(
        [bid0, bid1, bid2, bid3],
        ids,
        active,
        units,
        reserve,
        budget,
        processing_fee,
    );
    (result.price, result.sold)
}

fn fixture_ids() -> [[u8; 32]; N] {
    let mut ids = [[0; 32]; N];
    for i in 0..N {
        ids[i][31] = i as u8;
    }
    ids
}

#[test]
fn oversubscribed() {
    let s = auction(
        [90, 70, 60, 50],
        fixture_ids(),
        [true; N],
        3,
        20,
        100,
        15000,
    );
    assert(s.price == 50);
    assert(s.sold == 3);
    assert(s.seller_amount == 150);
    assert(s.refunds == [50, 50, 50, 100]);
    assert(s.network_fee == 30000);
    assert(s.operator_amount == 30000);
}

#[test]
fn tied_prices_use_last_id_byte() {
    let s = auction([50; N], fixture_ids(), [true; N], 2, 20, 100, 15000);
    assert(s.sold == 2);
    assert(s.refunds == [50, 50, 100, 100]);
}

#[test]
fn insufficient_demand() {
    let s = auction(
        [90, 10, 0, 0],
        fixture_ids(),
        [true, true, false, false],
        3,
        20,
        100,
        15000,
    );
    assert(s.price == 20);
    assert(s.sold == 1);
    assert(s.seller_amount == 20);
    assert(s.refunds == [80, 100, 0, 0]);
    assert(s.network_fee == 20000);
}

#[test]
fn empty() {
    let s = auction([0; N], fixture_ids(), [false; N], 3, 20, 100, 15000);
    assert(s.sold == 0);
    assert(s.seller_amount == 0);
    assert(s.network_fee == 0);
    assert(s.operator_amount == 0);
}

#[test(should_fail)]
fn duplicate_ticket_rejected() {
    let _s = auction([50; N], [[0; 32]; N], [true; N], 2, 20, 100, 15000);
}

#[test(should_fail)]
fn excessive_bid_rejected() {
    let _s = auction(
        [101, 50, 40, 30],
        fixture_ids(),
        [true; N],
        2,
        20,
        100,
        15000,
    );
}

#[test(should_fail)]
fn fee_shortfall_rejected() {
    let _s = auction(
        [50, 0, 0, 0],
        fixture_ids(),
        [true, false, false, false],
        2,
        20,
        100,
        0,
    );
}

#[test(should_fail)]
fn reserve_above_budget_rejected() {
    let _s = auction(
        [90, 70, 60, 50],
        fixture_ids(),
        [true; N],
        3,
        101,
        100,
        15000,
    );
}
