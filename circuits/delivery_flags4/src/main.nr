// Operator-only fill delivery. Public masked amounts use fresh uniform private
// field masks, authenticated to the exact auction context/root/ticket.
use darkcall::{auction, context, verify_accepted, limbs, hash, id_less};
global D_WINNER: Field = 0x4461726b43616c6c2f77696e6e65722d6d61736b2f7632;
global D_MASK: Field = 0x4461726b43616c6c2f66696c6c2d6d61736b2f7632;
pub fn mask_commitment(ctx: Field, root: Field, id: [u8; 32], mask: Field, blind: Field) -> Field {
    let ticket=limbs(id);
    hash([D_MASK,ctx,root,ticket[0],ticket[1],ticket[2],ticket[3],mask,blind])
}
pub fn winner_mask_commitment(ctx: Field, root: Field, id: [u8; 32], mask: Field, blind: Field) -> Field {
    let ticket = limbs(id);
    hash([D_WINNER,ctx,root,ticket[0],ticket[1],ticket[2],ticket[3],mask,blind])
}
// All four slots are authenticated/active in this fixed-size delivery circuit.
// Explicit flags are necessary when price is zero and all refunds are equal.
fn winning_flags(bids: [u64;4], ids: [[u8;32];4], units: u32, reserve: u64) -> [bool;4] {
    let mut flags = [false;4];
    for i in 0..4 {
        let mut rank = 0;
        for j in 0..4 {
            let ahead = (bids[j] > bids[i]) | ((bids[j] == bids[i]) & id_less(ids[j],ids[i]));
            if (bids[j] >= reserve) & ahead { rank += 1; }
        }
        flags[i] = (bids[i] >= reserve) & (rank < units);
    }
    flags
}
fn main(
    bid0:u64,bid1:u64,bid2:u64,bid3:u64,salt0:Field,salt1:Field,salt2:Field,salt3:Field,
    output_masks:[Field;4],output_blinds:[Field;4],winner_masks:[Field;4],winner_blinds:[Field;4],
    condition_digest:pub [u8;32],ids:pub [[u8;32];4],units:pub u32,
    reserve:pub u64,budget:pub u64,processing_fee:pub u64,commitments:pub [Field;4],root:pub Field,
    mask_commitments:pub [Field;4],winner_mask_commitments:pub [Field;4],
) -> pub (u64,u32,[Field;4],[Field;4]) {
    let ctx=context(condition_digest,units,reserve,budget,processing_fee);
    let bids=[bid0,bid1,bid2,bid3];
    verify_accepted(bids,[salt0,salt1,salt2,salt3],ids,[true;4],ctx,commitments,root);
    let result=auction(bids,ids,[true;4],units,reserve,budget,processing_fee);
    let mut masked=[0;4];
    let flags = winning_flags(bids,ids,units,reserve);
    let mut masked_flags = [0;4];
    let mut won_count = 0;
    for i in 0..4 {
        assert(mask_commitment(ctx,root,ids[i],output_masks[i],output_blinds[i])==mask_commitments[i]);
        masked[i]=result.refunds[i] as Field + output_masks[i];
        assert(winner_mask_commitment(ctx,root,ids[i],winner_masks[i],winner_blinds[i])==winner_mask_commitments[i]);
        masked_flags[i] = flags[i] as Field + winner_masks[i];
        won_count += flags[i] as u32;
        assert(result.refunds[i] == budget - if flags[i] { result.price } else { 0 });
    }
    assert(won_count == result.sold);
    (result.price,result.sold,masked,masked_flags)
}

#[test]
fn zero_price_equal_refunds_keep_tie_broken_winners() {
    let ids = [[1;32],[2;32],[3;32],[4;32]];
    let result = auction([0;4],ids,[true;4],3,0,100,15000);
    assert(result.price == 0);
    assert(result.refunds == [100;4]);
    assert(winning_flags([0;4],ids,3,0) == [true,true,true,false]);
}
#[test]
fn qualifications_ties_and_short_demand_match_auction() {
    let ids = [[1;32],[2;32],[3;32],[4;32]];
    assert(winning_flags([50;4],ids,3,20) == [true,true,true,false]);
    assert(winning_flags([10,30,5,2],ids,3,20) == [false,true,false,false]);
    assert(winning_flags([10;4],ids,3,20) == [false;4]);
}
#[test]
fn flag_masks_cannot_substitute_refund_masks_or_context() {
    let id = [7;32];
    let flag = winner_mask_commitment(1,2,id,3,9);
    assert(flag != winner_mask_commitment(1,2,id,3,10));
    assert(flag != mask_commitment(1,2,id,3,9));
    assert(flag != winner_mask_commitment(2,2,id,3,9));
    assert(flag != winner_mask_commitment(1,3,id,3,9));
    assert(flag != winner_mask_commitment(1,2,[8;32],3,9));
}
