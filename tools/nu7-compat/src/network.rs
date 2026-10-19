//! The only two chains this prototype may touch, pinned by genesis hash. Every
//! wallet capture, note file and admission context carries its genesis (and the
//! NU7 height that genesis implies); anything else is refused. Mainnet is not a
//! profile and cannot be selected.

pub const REGTEST_GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";
pub const TESTNET_GENESIS: &str = "05a60a92d99d85997cce3b87616c089f6124d7342af37106edc76126334a2c38";
pub const BRANCH: &str = "77190ad9";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Chain {
    /// The owned local Zebra regtest with NU7 at height 2.
    Regtest,
    /// Standard public testnet, read through a lightwalletd endpoint.
    Testnet,
}

impl Chain {
    pub fn from_genesis(genesis: &str) -> crate::Result<Self> {
        match genesis {
            REGTEST_GENESIS => Ok(Chain::Regtest),
            TESTNET_GENESIS => Ok(Chain::Testnet),
            _ => Err("genesis is neither the pinned regtest nor standard testnet".into()),
        }
    }
    pub fn from_genesis_bytes(genesis: &[u8; 32]) -> crate::Result<Self> {
        Chain::from_genesis(&hex::encode(genesis))
    }
    pub fn genesis(self) -> &'static str {
        match self {
            Chain::Regtest => REGTEST_GENESIS,
            Chain::Testnet => TESTNET_GENESIS,
        }
    }
    pub fn genesis_bytes(self) -> [u8; 32] {
        hex::decode(self.genesis()).expect("pinned genesis").try_into().expect("32 bytes")
    }
    pub fn nu7_height(self) -> u32 {
        match self {
            Chain::Regtest => 2,
            Chain::Testnet => crate::testnet::NU7_HEIGHT,
        }
    }
    /// Scope label written into, and required from, scanned note files.
    pub fn openings_scope(self) -> &'static str {
        match self {
            Chain::Regtest => "isolated NU7 regtest wallet openings",
            Chain::Testnet => "public NU7 testnet wallet openings",
        }
    }
    /// Validates a (genesis, branch, NU7 height) triple read from a data file.
    pub fn checked(genesis: &str, branch: &str, nu7_height: u32) -> crate::Result<Self> {
        let chain = Chain::from_genesis(genesis)?;
        if branch != BRANCH || nu7_height != chain.nu7_height() {
            return Err("network branch or NU7 height differs from the pinned profile".into());
        }
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_two_pinned_profiles() {
        assert_eq!(Chain::checked(REGTEST_GENESIS, BRANCH, 2).unwrap(), Chain::Regtest);
        assert_eq!(Chain::checked(TESTNET_GENESIS, BRANCH, 4_465_026).unwrap(), Chain::Testnet);
        assert!(Chain::checked(TESTNET_GENESIS, BRANCH, 2).is_err());
        assert!(Chain::checked(REGTEST_GENESIS, "77190ad8", 2).is_err());
        // Mainnet genesis is not a profile.
        assert!(Chain::from_genesis("00040fe8ec8471911baa1db1266ea15dd06b4a8a5c453883c000b031973dce08").is_err());
    }
}
