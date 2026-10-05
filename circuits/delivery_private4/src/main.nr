// Operator-only fill delivery. Public masked amounts use fresh uniform private
// field masks, authenticated to the exact auction context/root/ticket.
use darkcall::{auction, context, verify_accepted, limbs, hash};
global D_MASK: Field = 0x4461726b43616c6c2f66696c6c2d6d61736b2f7632;
pub fn mask_commitment(ctx: Field, root: Field, id: [u8; 32], mask: Field, blind: Field) -> Field {
    let ticket=limbs(id);
    hash([D_MASK,ctx,root,ticket[0],ticket[1],ticket[2],ticket[3],mask,blind])
}
fn main(
    bid0:u64,bid1:u64,bid2:u64,bid3:u64,salt0:Field,salt1:Field,salt2:Field,salt3:Field,
    output_masks:[Field;4],output_blinds:[Field;4],
    condition_digest:pub [u8;32],ids:pub [[u8;32];4],units:pub u32,
    reserve:pub u64,budget:pub u64,processing_fee:pub u64,commitments:pub [Field;4],root:pub Field,
    mask_commitments:pub [Field;4],
) -> pub (u64,u32,[Field;4]) {
    let ctx=context(condition_digest,units,reserve,budget,processing_fee);
    let bids=[bid0,bid1,bid2,bid3];
    verify_accepted(bids,[salt0,salt1,salt2,salt3],ids,[true;4],ctx,commitments,root);
    let result=auction(bids,ids,[true;4],units,reserve,budget,processing_fee);
    let mut masked=[0;4];
    for i in 0..4 {
        assert(mask_commitment(ctx,root,ids[i],output_masks[i],output_blinds[i])==mask_commitments[i]);
        masked[i]=result.refunds[i] as Field + output_masks[i];
    }
    (result.price,result.sold,masked)
}
