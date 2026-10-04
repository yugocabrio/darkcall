//! Plaintext correctness oracle for synthetic/offline tests.
//!
//! This module deliberately knows every bid. It is NOT a confidential auction
//! engine, and must not be used as the live MPC auction service.

pub mod funding;

use std::collections::BTreeSet;

pub const MAX_BIDS: usize = 32;
pub const MAX_MONEY: u64 = 21_000_000 * 100_000_000;
pub type TicketId = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    pub units: u32,
    pub reserve: u64,
    pub budget: u64,
    pub processing_fee: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bid {
    pub ticket_id: TicketId,
    pub amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refund {
    pub ticket_id: TicketId,
    pub won: bool,
    pub charged: u64,
    pub refunded: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settlement {
    pub accepted: u32,
    pub sold: u32,
    pub price: u64,
    pub action_count: u32,
    pub network_fee: u64,
    pub seller_amount: u64,
    pub operator_amount: u64,
    /// Canonical ascending unsigned-byte ticket order, not ranking order.
    pub refunds: Vec<Refund>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidTerms,
    TooManyBids,
    DuplicateTicket,
    BidExceedsBudget,
    InsufficientProcessingFees,
    MoneyRange,
}

/// DarkCall's proposed Ironwood-only n+2-output shape; native feasibility is
/// a separate integration gate. ZIP 317 is a wallet convention, not consensus.
pub fn normal_fee(count: usize) -> Result<(u32, u64), Error> {
    if count > MAX_BIDS {
        return Err(Error::TooManyBids);
    }
    if count == 0 {
        return Ok((0, 0));
    }
    let actions = count as u32 + 2;
    Ok((actions, 5_000 * u64::from(actions.max(2))))
}

fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b)
        .filter(|n| *n <= MAX_MONEY)
        .ok_or(Error::MoneyRange)
}

fn mul(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_mul(b)
        .filter(|n| *n <= MAX_MONEY)
        .ok_or(Error::MoneyRange)
}

pub fn clear(terms: Terms, bids: &[Bid]) -> Result<Settlement, Error> {
    if terms.units == 0 || terms.units as usize > MAX_BIDS || terms.reserve > terms.budget {
        return Err(Error::InvalidTerms);
    }
    let deposit = add(terms.budget, terms.processing_fee)?;
    let (action_count, network_fee) = normal_fee(bids.len())?;
    let total_input = mul(deposit, bids.len() as u64)?;
    let processing_total = mul(terms.processing_fee, bids.len() as u64)?;
    let operator_amount = processing_total
        .checked_sub(network_fee)
        .ok_or(Error::InsufficientProcessingFees)?;

    let mut ids = BTreeSet::new();
    for bid in bids {
        if bid.amount > terms.budget {
            return Err(Error::BidExceedsBudget);
        }
        if !ids.insert(bid.ticket_id) {
            return Err(Error::DuplicateTicket);
        }
    }

    let mut qualified: Vec<&Bid> = bids
        .iter()
        .filter(|bid| bid.amount >= terms.reserve)
        .collect();
    qualified.sort_by(|a, b| b.amount.cmp(&a.amount).then(a.ticket_id.cmp(&b.ticket_id)));
    let sold = qualified.len().min(terms.units as usize);
    let price = if qualified.len() > terms.units as usize {
        terms.reserve.max(qualified[terms.units as usize].amount)
    } else {
        terms.reserve
    };
    let winners: BTreeSet<TicketId> = qualified[..sold].iter().map(|bid| bid.ticket_id).collect();
    let seller_amount = mul(sold as u64, price)?;
    let mut refunds: Vec<Refund> = bids
        .iter()
        .map(|bid| {
            let won = winners.contains(&bid.ticket_id);
            let charged = if won { price } else { 0 };
            Refund {
                ticket_id: bid.ticket_id,
                won,
                charged,
                refunded: terms.budget - charged,
            }
        })
        .collect();
    refunds.sort_by_key(|row| row.ticket_id);
    let refunded = refunds
        .iter()
        .try_fold(0, |sum, row| add(sum, row.refunded))?;
    let output_and_fee = add(
        add(refunded, seller_amount)?,
        add(operator_amount, network_fee)?,
    )?;
    if output_and_fee != total_input {
        return Err(Error::MoneyRange);
    }

    Ok(Settlement {
        accepted: bids.len() as u32,
        sold: sold as u32,
        price,
        action_count,
        network_fee,
        seller_amount,
        operator_amount,
        refunds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bid(id: u8, amount: u64) -> Bid {
        let mut ticket_id = [0; 32];
        ticket_id[31] = id;
        Bid { ticket_id, amount }
    }

    fn terms() -> Terms {
        Terms {
            units: 3,
            reserve: 20_000_000,
            budget: 100_000_000,
            processing_fee: 15_000,
        }
    }

    #[test]
    fn documented_example_conserves_every_zatoshi() {
        let bids: Vec<_> = [90, 70, 60, 50, 30, 10]
            .into_iter()
            .enumerate()
            .map(|(i, amount)| bid(i as u8, amount * 1_000_000))
            .collect();
        let s = clear(terms(), &bids).unwrap();
        assert_eq!(
            (s.sold, s.price, s.seller_amount),
            (3, 50_000_000, 150_000_000)
        );
        assert_eq!(
            s.refunds.iter().map(|r| r.refunded).sum::<u64>(),
            450_000_000
        );
        assert_eq!(
            (s.action_count, s.network_fee, s.operator_amount),
            (8, 40_000, 50_000)
        );
    }

    #[test]
    fn insufficient_demand_pays_reserve_only_for_sold_units() {
        let s = clear(terms(), &[bid(1, 80_000_000), bid(2, 10_000_000)]).unwrap();
        assert_eq!(
            (s.sold, s.price, s.seller_amount),
            (1, 20_000_000, 20_000_000)
        );
        assert_eq!(s.refunds[1].refunded, 100_000_000);
    }

    #[test]
    fn empty_or_no_eligible_bids_have_no_seller_payment() {
        let empty = clear(terms(), &[]).unwrap();
        assert_eq!(
            (
                empty.sold,
                empty.network_fee,
                empty.operator_amount,
                empty.action_count
            ),
            (0, 0, 0, 0)
        );
        let s = clear(terms(), &[bid(0, 1), bid(1, 2)]).unwrap();
        assert_eq!((s.sold, s.seller_amount), (0, 0));
        assert!(s.refunds.iter().all(|r| r.refunded == terms().budget));
    }

    #[test]
    fn tie_uses_all_32_bytes_and_unsigned_order() {
        let t = Terms {
            units: 1,
            ..terms()
        };
        let mut high = [0; 32];
        high[0] = 0x80;
        let low = [0x7f; 32];
        let bids = [
            Bid {
                ticket_id: high,
                amount: 50_000_000,
            },
            Bid {
                ticket_id: low,
                amount: 50_000_000,
            },
        ];
        let s = clear(t, &bids).unwrap();
        assert_eq!(s.refunds[0].ticket_id, low);
        assert!(s.refunds[0].won);
        assert!(!s.refunds[1].won);
        let last_byte = clear(t, &[bid(2, 50_000_000), bid(1, 50_000_000)]).unwrap();
        assert!(last_byte.refunds[0].won);
        assert_eq!(last_byte.refunds[0].ticket_id[31], 1);
    }

    #[test]
    fn input_permutation_does_not_change_canonical_settlement() {
        let bids = [
            bid(3, 90_000_000),
            bid(0, 70_000_000),
            bid(2, 60_000_000),
            bid(1, 50_000_000),
        ];
        let expected = clear(terms(), &bids).unwrap();
        for a in 0..4 {
            for b in 0..4 {
                if a == b {
                    continue;
                }
                for c in 0..4 {
                    if c == a || c == b {
                        continue;
                    }
                    let d = (0..4).find(|i| *i != a && *i != b && *i != c).unwrap();
                    assert_eq!(
                        clear(terms(), &[bids[a], bids[b], bids[c], bids[d]]).unwrap(),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn reject_invalid_terms_inputs_and_funding() {
        assert_eq!(
            clear(
                Terms {
                    units: 0,
                    ..terms()
                },
                &[]
            ),
            Err(Error::InvalidTerms)
        );
        assert_eq!(
            clear(
                Terms {
                    reserve: 100_000_001,
                    ..terms()
                },
                &[]
            ),
            Err(Error::InvalidTerms)
        );
        assert_eq!(
            clear(terms(), &[bid(0, 100_000_001)]),
            Err(Error::BidExceedsBudget)
        );
        assert_eq!(
            clear(terms(), &[bid(0, 1), bid(0, 2)]),
            Err(Error::DuplicateTicket)
        );
        assert_eq!(
            clear(
                Terms {
                    processing_fee: 0,
                    ..terms()
                },
                &[bid(0, 1)]
            ),
            Err(Error::InsufficientProcessingFees)
        );
        assert_eq!(clear(terms(), &[bid(0, 1); 33]), Err(Error::TooManyBids));
        assert_eq!(
            clear(
                Terms {
                    budget: u64::MAX,
                    ..terms()
                },
                &[]
            ),
            Err(Error::MoneyRange)
        );
        assert_eq!(
            clear(
                Terms {
                    budget: MAX_MONEY - 15_000,
                    ..terms()
                },
                &[bid(0, 1), bid(1, 1)]
            ),
            Err(Error::MoneyRange)
        );
    }

    #[test]
    fn small_books_obey_incentive_and_conservation_invariants() {
        for units in 1..=4 {
            for reserve in 0..=3 {
                let t = Terms {
                    units,
                    reserve,
                    budget: 3,
                    processing_fee: 15_000,
                };
                for encoded in 0_u32..256 {
                    let bids: Vec<_> = (0..4)
                        .map(|i| bid(i, u64::from((encoded >> (2 * i)) & 3)))
                        .collect();
                    let s = clear(t, &bids).unwrap();
                    let eligible = bids.iter().filter(|b| b.amount >= reserve).count();
                    assert_eq!(s.sold as usize, eligible.min(units as usize));
                    assert_eq!(s.seller_amount, u64::from(s.sold) * s.price);
                    assert!(s.price >= reserve && s.price <= t.budget);
                    for row in &s.refunds {
                        let b = bids.iter().find(|b| b.ticket_id == row.ticket_id).unwrap();
                        assert_eq!(row.refunded + row.charged, t.budget);
                        if row.won {
                            assert!(b.amount >= s.price);
                        } else if eligible > units as usize && b.amount >= reserve {
                            assert!(b.amount <= s.price);
                        }
                    }
                    let out = s.refunds.iter().map(|r| r.refunded).sum::<u64>()
                        + s.seller_amount
                        + s.operator_amount
                        + s.network_fee;
                    assert_eq!(out, 4 * (t.budget + t.processing_fee));
                }
            }
        }
    }
}
