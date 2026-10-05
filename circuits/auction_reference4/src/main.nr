// PLAINTEXT SYNTHETIC CORRECTNESS HARNESS. No MPC, ZK proof or privacy claim.
use darkcall::auction;
fn main(
    bids: [u64;4], ids: [[u8;32];4], active: [bool;4],
    units: u32, reserve: u64, budget: u64, processing_fee: u64,
    expected: [u64;10], require_match: pub bool,
) -> pub [u64;10] {
    let r = auction(bids,ids,active,units,reserve,budget,processing_fee);
    let result = [r.price,r.sold as u64,r.seller_amount,r.operator_amount,
                  r.network_fee,r.action_count as u64,
                  r.refunds[0],r.refunds[1],r.refunds[2],r.refunds[3]];
    if require_match { assert(result == expected); }
    result
}
