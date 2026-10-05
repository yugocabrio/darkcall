//! Private wallet integration primitives. No keys or maximum bids belong in
//! the funding journal. Scanner observations remain trusted input.
pub mod claim;
pub mod journal;
pub mod receipt;
pub mod scanner;
