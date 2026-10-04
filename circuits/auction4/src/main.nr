// Initial computation gate only. No funding/acceptance commitments or native
// settlement binding yet. Proving this circuit is not the completed protocol.
global N: u32 = 4;
global MAX_MONEY: u64 = 2100000000000000;

struct Result {
    price: u64,
    sold: u32,
    seller_amount: u64,
    operator_amount: u64,
    network_fee: u64,
    action_count: u32,
    refunds: [u64; N],
}

fn id_less(a: [u8; 32], b: [u8; 32]) -> bool {
    let mut less = false;
    let mut equal = true;
    for i in 0..32 {
        less = less | (equal & (a[i] < b[i]));
        equal = equal & (a[i] == b[i]);
    }
    less
}

fn auction(
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

fn main(
    bid0: u64,
    bid1: u64,
    bid2: u64,
    bid3: u64,
    ids: pub [[u8; 32]; N],
    active: pub [bool; N],
    units: pub u32,
    reserve: pub u64,
    budget: pub u64,
    processing_fee: pub u64,
) -> pub (u64, u32) {
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
