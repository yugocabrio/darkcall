//! Shared off-chain intent/receipt/final-book/journal rules. No native transaction
//! SDK or scanner dependency. Source modules are reused, never independently
//! reimplemented; native adapters must supply actual decryption and chain checks.
#[path = "../../wallet-bridge/src/claim_intent.rs"]
pub mod claim;
#[path = "../../wallet-bridge/src/final_book.rs"]
pub mod final_book;
#[path = "../../wallet-bridge/src/journal.rs"]
pub mod journal;
#[path = "../../wallet-bridge/src/receipt.rs"]
pub mod receipt;
