// Four-funded-ticket settlement statement. Compile gate first; authenticated
// funding/acceptance and operator-only delivery of fills are separate work.
use darkcall::{auction, context, verify_accepted, limbs, hash, bid_commitment, accepted_root};
use rows::{Row, encode};

// Candidate uses the exact existing BN254 Poseidon2 fixed sponge.
pub fn commitment(row: Row) -> [u32; 8] {
    let input = encode(row);
    let mut packed = [0; 8];
    for i in 0..8 { for j in 0..30 {
        let index = i * 30 + j;
        if index < 224 { packed[i] = packed[i] * 256 + input[index] as Field; }
    } }
    let bytes: [u8; 32] = hash(packed).to_be_bytes();
    let mut words = [0; 8];
    for i in 0..8 { for j in 0..4 { words[i] = words[i] * 256 + bytes[i*4+j] as u32; } }
    words
}


global D_RECEIVER: Field = 0x4461726b43616c6c2f72656365697665722f7631;
global D_PLAN: Field = 0x4461726b43616c6c2f706c616e2f7631;

// The funding/conditions receipt must authenticate these receiver commitments.
// This circuit checks the private opening, not the receipt issuer's authority.
pub fn receiver_commitment(ctx: Field, ticket: [u8; 32], role: u32, gd: [u8; 32], pkd: [u8; 32], nonce: Field) -> Field {
    let id = limbs(ticket); let g = limbs(gd); let p = limbs(pkd);
    hash([D_RECEIVER, ctx, id[0], id[1], id[2], id[3], role as Field, g[0], g[1], g[2], g[3], p[0], p[1], p[2], p[3], nonce])
}

fn main(
    bid0: u64, bid1: u64, bid2: u64, bid3: u64,
    salt0: Field, salt1: Field, salt2: Field, salt3: Field,
    gd: [[u8; 32]; 6], pkd: [[u8; 32]; 6], receiver_nonces: [Field; 6], row_salts: [[u8; 32]; 6],
    condition_digest: pub [u8; 32], ids: pub [[u8; 32]; 4], units: pub u32,
    reserve: pub u64, budget: pub u64, processing_fee: pub u64,
    commitments: pub [Field; 4], root: pub Field,
    receivers: pub [Field; 6], action_indices: pub [u32; 6],
) -> pub (u64, u32, Field, [[u32; 8]; 6]) {
    let ctx = context(condition_digest, units, reserve, budget, processing_fee);
    let bids = [bid0, bid1, bid2, bid3];
    verify_accepted(bids, [salt0, salt1, salt2, salt3], ids, [true; 4], ctx, commitments, root);
    let result = auction(bids, ids, [true; 4], units, reserve, budget, processing_fee);
    let amounts = [result.refunds[0], result.refunds[1], result.refunds[2], result.refunds[3], result.seller_amount, result.operator_amount];
    let root_bytes: [u8; 32] = root.to_be_bytes();
    let mut digests = [[0; 8]; 6];
    for i in 0..6 {
        assert(action_indices[i] < 6);
        for j in 0..i { assert(action_indices[j] != action_indices[i]); }
        let role = if i < 4 { 0 } else { i - 3 };
        let ticket = if i < 4 { ids[i] } else { [0; 32] };
        assert(receiver_commitment(ctx, ticket, role, gd[i], pkd[i], receiver_nonces[i]) == receivers[i]);
        digests[i] = commitment(Row {
            conditions: condition_digest, accepted_root: root_bytes, ticket,
            role, action_index: action_indices[i], gd: gd[i], pkd: pkd[i],
            amount: amounts[i], salt: row_salts[i],
        });
    }
    let mut plan_fields = [0; 56];
    plan_fields[0] = D_PLAN; plan_fields[1] = ctx; plan_fields[2] = root;
    plan_fields[3] = result.price as Field; plan_fields[4] = result.sold as Field;
    plan_fields[5] = result.action_count as Field; plan_fields[6] = result.network_fee as Field;
    plan_fields[7] = result.operator_amount as Field;
    for i in 0..6 { for j in 0..8 { plan_fields[8 + i * 8 + j] = digests[i][j] as Field; } }
    (result.price, result.sold, hash(plan_fields), digests)
}

#[test]
fn receiver_commitment_binds_role_and_ticket() {
    let original = receiver_commitment(1, [2; 32], 0, [3; 32], [4; 32], 5);
    assert(original != receiver_commitment(1, [2; 32], 1, [3; 32], [4; 32], 5));
    assert(original != receiver_commitment(1, [0; 32], 0, [3; 32], [4; 32], 5));
    assert(original != receiver_commitment(2, [2; 32], 0, [3; 32], [4; 32], 5));
}

fn run_fixture(action_indices: [u32; 6], changed_recipient: bool) {
    let conditions = [1; 32];
    let ids = [[1; 32], [2; 32], [3; 32], [4; 32]];
    let bids = [90, 70, 60, 50];
    let bid_salts = [100, 101, 102, 103];
    let ctx = context(conditions, 3, 20, 100, 15000);
    let mut leaves = [0; 4];
    for i in 0..4 { leaves[i] = bid_commitment(ctx, ids[i], bids[i], bid_salts[i]); }
    let root = accepted_root(ctx, [true; 4], leaves);
    let gd = [[3; 32]; 6]; let mut pkd = [[4; 32]; 6];
    let nonces = [1, 2, 3, 4, 5, 6]; let salts = [[7; 32]; 6];
    let mut receivers = [0; 6];
    for i in 0..6 {
        let role = if i < 4 { 0 } else { i - 3 };
        let ticket = if i < 4 { ids[i] } else { [0; 32] };
        receivers[i] = receiver_commitment(ctx, ticket, role, gd[i], pkd[i], nonces[i]);
    }
    if changed_recipient { pkd[0][0] = 5; }
    let (price, sold, _plan, digests) = main(
        bids[0], bids[1], bids[2], bids[3], bid_salts[0], bid_salts[1], bid_salts[2], bid_salts[3],
        gd, pkd, nonces, salts, conditions, ids, 3, 20, 100, 15000, leaves, root, receivers, action_indices,
    );
    assert(price == 50); assert(sold == 3);
    let amounts = [50, 50, 50, 100, 150, 30000];
    for i in 0..6 {
        assert(digests[i] == commitment(Row {
            conditions, accepted_root: root.to_be_bytes(),
            ticket: if i < 4 { ids[i] } else { [0; 32] },
            role: if i < 4 { 0 } else { i - 3 }, action_index: action_indices[i],
            gd: gd[i], pkd: pkd[i], amount: amounts[i], salt: salts[i],
        }));
    }
}

#[test]
fn derived_amounts_and_permuted_actions_are_committed() { run_fixture([5, 2, 0, 4, 1, 3], false); }

#[test(should_fail)]
fn reused_action_is_rejected() { run_fixture([0, 1, 2, 3, 4, 4], false); }

#[test(should_fail)]
fn changed_receiver_opening_is_rejected() { run_fixture([0, 1, 2, 3, 4, 5], true); }
