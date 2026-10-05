//! Plaintext Rust oracle for fabricated committed-input MPC test cases only.
//! This executable is never part of the private auction or native signer path.
use darkcall_auction_core::{Bid, Terms, clear};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .map(|v| v.parse())
        .collect::<Result<_, _>>()?;
    if args.len() != 5 || ![0, 20].contains(&args[0]) || args[1..].iter().any(|b| *b > 100) {
        return Err(
            "synthetic oracle requires RESERVE BID0 BID1 BID2 BID3 within budget100".into(),
        );
    }
    let bids: [Bid; 4] = std::array::from_fn(|i| {
        let mut id = [0; 32];
        id[31] = i as u8;
        Bid {
            ticket_id: id,
            amount: args[i + 1],
        }
    });
    let result = clear(
        Terms {
            units: 3,
            reserve: args[0],
            budget: 100,
            processing_fee: 15000,
        },
        &bids,
    )
    .map_err(|_| "invalid fabricated oracle case")?;
    let amounts = format!(
        "{},{},{}",
        result
            .refunds
            .iter()
            .map(|r| r.refunded.to_string())
            .collect::<Vec<_>>()
            .join(","),
        result.seller_amount,
        result.operator_amount
    );
    let winners = result
        .refunds
        .iter()
        .map(|r| r.won.to_string())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"scope\":\"plaintext synthetic Rust sort oracle, not MPC\",\"price\":{},\"sold\":{},\"amounts\":[{}],\"winners\":[{}],\"network_fee\":{}}}",
        result.price, result.sold, amounts, winners, result.network_fee
    );
    Ok(())
}
