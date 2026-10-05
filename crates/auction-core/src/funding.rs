//! Private funding ledger state, fed by a trusted wallet/node scanner. It is not
//! a public funding proof or authenticated receipt. Never publish note IDs.
use crate::{MAX_BIDS, MAX_MONEY, Terms, TicketId, clear};
use std::collections::{BTreeMap, BTreeSet};

pub type NoteId = [u8; 32];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Accepting,
    Frozen,
    ReorgPaused,
    Prepared,
    Broadcast,
    Finalized,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Settlement,
    Cancellation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Pending,
    Eligible,
    WrongAmount,
    Late,
    Accepted,
    Reorged,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidTerms,
    InvalidObservation,
    ConflictingObservation,
    UnknownNote,
    NotConfirmed,
    WrongAmount,
    Late,
    DuplicateTicket,
    NoteAlreadyAssigned,
    TooManyTickets,
    WrongPhase,
    ChangedTransaction,
    NoAcceptedTickets,
}

#[derive(Clone)]
pub struct Observation {
    pub note: NoteId,
    pub value: u64,
    pub block: [u8; 32],
    pub height: u32,
}
#[derive(Clone)]
pub struct Registration {
    pub ticket: TicketId,
    pub note: NoteId,
    pub bid_commitment: [u8; 32],
    pub receiver_commitment: [u8; 32],
}
#[derive(Clone)]
struct Deposit {
    observed: Observation,
    disposition: Disposition,
    confirmed: bool,
}

#[derive(Clone)]
pub struct Ledger {
    required: u64,
    phase: Phase,
    notes: BTreeMap<NoteId, Deposit>,
    tickets: BTreeMap<TicketId, Registration>,
    transaction: Option<([u8; 32], Purpose)>,
}
impl Ledger {
    pub fn new(terms: Terms) -> Result<Self, Error> {
        clear(terms, &[]).map_err(|_| Error::InvalidTerms)?;
        let required = terms
            .budget
            .checked_add(terms.processing_fee)
            .filter(|v| *v <= MAX_MONEY)
            .ok_or(Error::InvalidTerms)?;
        Ok(Self {
            required,
            phase: Phase::Accepting,
            notes: BTreeMap::new(),
            tickets: BTreeMap::new(),
            transaction: None,
        })
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn registration(&self, ticket: &TicketId) -> Option<&Registration> {
        self.tickets.get(ticket)
    }
    /// Idempotent scan replay; a same-note observation with changed value/block
    /// must go through explicit reorg handling, never overwrite accepted funding.
    pub fn observe(&mut self, observed: Observation) -> Result<Disposition, Error> {
        if observed.value > MAX_MONEY {
            return Err(Error::InvalidObservation);
        }
        if let Some(existing) = self.notes.get(&observed.note) {
            let old = &existing.observed;
            if old.value != observed.value
                || old.block != observed.block
                || old.height != observed.height
            {
                return Err(Error::ConflictingObservation);
            }
            return Ok(existing.disposition);
        }
        let disposition = if observed.value != self.required {
            Disposition::WrongAmount
        } else if self.phase != Phase::Accepting {
            Disposition::Late
        } else {
            Disposition::Pending
        };
        self.notes.insert(
            observed.note,
            Deposit {
                observed,
                disposition,
                confirmed: false,
            },
        );
        Ok(disposition)
    }
    /// The scanner must have checked this block belongs to the canonical chain.
    pub fn confirm(&mut self, note: &NoteId, block: &[u8; 32]) -> Result<Disposition, Error> {
        let deposit = self.notes.get_mut(note).ok_or(Error::UnknownNote)?;
        if &deposit.observed.block != block || deposit.disposition == Disposition::Reorged {
            return Err(Error::InvalidObservation);
        }
        deposit.confirmed = true;
        if deposit.disposition == Disposition::Pending {
            deposit.disposition = Disposition::Eligible;
        }
        Ok(deposit.disposition)
    }
    /// Call only after client ZK proof and all delivered-share checks passed.
    /// Receipt authentication is a separate layer; this API asserts none.
    pub fn accept_validated(&mut self, registration: Registration) -> Result<(), Error> {
        if self.phase != Phase::Accepting {
            return Err(Error::WrongPhase);
        }
        if self.tickets.contains_key(&registration.ticket) {
            return Err(Error::DuplicateTicket);
        }
        if self.tickets.values().any(|r| r.note == registration.note) {
            return Err(Error::NoteAlreadyAssigned);
        }
        if self.tickets.len() >= MAX_BIDS {
            return Err(Error::TooManyTickets);
        }
        let deposit = self
            .notes
            .get_mut(&registration.note)
            .ok_or(Error::UnknownNote)?;
        if !deposit.confirmed {
            return Err(Error::NotConfirmed);
        }
        match deposit.disposition {
            Disposition::Eligible => {}
            Disposition::WrongAmount => return Err(Error::WrongAmount),
            Disposition::Late => return Err(Error::Late),
            _ => return Err(Error::WrongPhase),
        }
        deposit.disposition = Disposition::Accepted;
        self.tickets.insert(registration.ticket, registration);
        Ok(())
    }
    pub fn freeze(&mut self) -> Result<Vec<Registration>, Error> {
        if self.phase != Phase::Accepting {
            return Err(Error::WrongPhase);
        }
        self.phase = Phase::Frozen;
        Ok(self.tickets.values().cloned().collect())
    }
    /// Leave every deposit recorded, including invalid/late/unaccepted ones.
    /// Wrong amounts need an explicit refund-fee policy, not the budget formula.
    pub fn refund_queue(&self) -> Vec<(NoteId, u64, Disposition)> {
        self.notes
            .iter()
            .filter(|(_, d)| {
                d.disposition != Disposition::Accepted && d.disposition != Disposition::Reorged
            })
            .map(|(id, d)| (*id, d.observed.value, d.disposition))
            .collect()
    }
    pub fn invalidate_chain_notes(&mut self, removed: &BTreeSet<NoteId>) -> Result<(), Error> {
        if matches!(
            self.phase,
            Phase::Prepared | Phase::Broadcast | Phase::Finalized
        ) {
            return Err(Error::WrongPhase);
        }
        if removed.iter().any(|n| !self.notes.contains_key(n)) {
            return Err(Error::UnknownNote);
        }
        for id in removed {
            let d = self.notes.get_mut(id).unwrap();
            d.disposition = Disposition::Reorged;
            d.confirmed = false;
        }
        self.tickets.retain(|_, r| !removed.contains(&r.note));
        if self.phase == Phase::Frozen {
            self.phase = Phase::ReorgPaused;
        }
        Ok(())
    }
    /// Reserves accepted inputs once, with no unsafe unlock/force-reset path.
    /// Reusing an expired transaction requires node-assisted recovery elsewhere.
    pub fn prepare(&mut self, effects: [u8; 32], purpose: Purpose) -> Result<(), Error> {
        if self.phase != Phase::Frozen {
            return Err(Error::WrongPhase);
        }
        if self.tickets.is_empty() {
            return Err(Error::NoAcceptedTickets);
        }
        self.transaction = Some((effects, purpose));
        self.phase = Phase::Prepared;
        Ok(())
    }
    pub fn mark_broadcast(&mut self, effects: &[u8; 32]) -> Result<(), Error> {
        if self.phase != Phase::Prepared {
            return Err(Error::WrongPhase);
        }
        if self.transaction.as_ref().map(|t| &t.0) != Some(effects) {
            return Err(Error::ChangedTransaction);
        }
        self.phase = Phase::Broadcast;
        Ok(())
    }
    pub fn mark_finalized(&mut self, effects: &[u8; 32]) -> Result<(), Error> {
        if self.phase != Phase::Broadcast {
            return Err(Error::WrongPhase);
        }
        if self.transaction.as_ref().map(|t| &t.0) != Some(effects) {
            return Err(Error::ChangedTransaction);
        }
        self.phase = Phase::Finalized;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ledger() -> Ledger {
        Ledger::new(Terms {
            units: 3,
            reserve: 20,
            budget: 100,
            processing_fee: 15000,
        })
        .unwrap()
    }
    fn observed(id: u8, value: u64) -> Observation {
        Observation {
            note: [id; 32],
            value,
            block: [8; 32],
            height: 10,
        }
    }
    fn register(id: u8, note: u8) -> Registration {
        Registration {
            ticket: [id; 32],
            note: [note; 32],
            bid_commitment: [3; 32],
            receiver_commitment: [4; 32],
        }
    }
    #[test]
    fn one_note_cannot_fund_two_tickets_and_scan_replay_is_idempotent() {
        let mut l = ledger();
        l.observe(observed(1, 15100)).unwrap();
        assert_eq!(l.accept_validated(register(1, 1)), Err(Error::NotConfirmed));
        l.confirm(&[1; 32], &[8; 32]).unwrap();
        l.accept_validated(register(1, 1)).unwrap();
        assert_eq!(l.observe(observed(1, 15100)), Ok(Disposition::Accepted));
        assert_eq!(
            l.accept_validated(register(2, 1)),
            Err(Error::NoteAlreadyAssigned)
        );
        assert_eq!(
            l.observe(observed(1, 15099)),
            Err(Error::ConflictingObservation)
        );
        assert_eq!(l.freeze().unwrap().len(), 1);
    }
    #[test]
    fn wrong_late_and_unaccepted_money_stays_in_refund_queue() {
        let mut l = ledger();
        l.observe(observed(1, 7)).unwrap();
        l.confirm(&[1; 32], &[8; 32]).unwrap();
        assert_eq!(l.accept_validated(register(1, 1)), Err(Error::WrongAmount));
        l.observe(observed(2, 15100)).unwrap();
        l.freeze().unwrap();
        assert_eq!(l.observe(observed(3, 15100)), Ok(Disposition::Late));
        let queue = l.refund_queue();
        assert_eq!(queue.len(), 3);
        assert_eq!(queue[0].1, 7);
    }
    #[test]
    fn frozen_root_cannot_settle_after_reorg() {
        let mut l = ledger();
        l.observe(observed(1, 15100)).unwrap();
        l.confirm(&[1; 32], &[8; 32]).unwrap();
        l.accept_validated(register(1, 1)).unwrap();
        l.freeze().unwrap();
        l.invalidate_chain_notes(&BTreeSet::from([[1; 32]]))
            .unwrap();
        assert_eq!(l.phase(), Phase::ReorgPaused);
        assert_eq!(
            l.prepare([4; 32], Purpose::Settlement),
            Err(Error::WrongPhase)
        );
    }
    #[test]
    fn settlement_and_cancellation_cannot_reserve_the_same_inputs() {
        let mut l = ledger();
        l.observe(observed(1, 15100)).unwrap();
        l.confirm(&[1; 32], &[8; 32]).unwrap();
        l.accept_validated(register(1, 1)).unwrap();
        l.freeze().unwrap();
        l.prepare([4; 32], Purpose::Settlement).unwrap();
        assert_eq!(
            l.prepare([5; 32], Purpose::Cancellation),
            Err(Error::WrongPhase)
        );
        assert_eq!(l.mark_broadcast(&[5; 32]), Err(Error::ChangedTransaction));
        l.mark_broadcast(&[4; 32]).unwrap();
        l.mark_finalized(&[4; 32]).unwrap();
        assert_eq!(
            l.prepare([5; 32], Purpose::Cancellation),
            Err(Error::WrongPhase)
        );
    }
    #[test]
    fn zero_ticket_auction_creates_no_native_transaction() {
        let mut l = ledger();
        l.freeze().unwrap();
        assert_eq!(
            l.prepare([4; 32], Purpose::Settlement),
            Err(Error::NoAcceptedTickets)
        );
    }
}
