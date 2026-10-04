//! Read-only network identification. No keys, funds or broadcast RPCs.
use std::time::Duration;
use tonic::transport::{ClientTlsConfig, Endpoint};
use zcash_client_backend::proto::service::{
    BlockId, ChainSpec, Empty, compact_tx_streamer_client::CompactTxStreamerClient,
};
use zcash_protocol::consensus::{BlockHeight, BranchId, TEST_NETWORK};

const TEST_GENESIS: &str = "05a60a92d99d85997cce3b87616c089f6124d7342af37106edc76126334a2c38";
const REGTEST_GENESIS: &str = "029f11d80ef9765602235e1bc9727e3eb6ba20839319f761fee920d63401e327";

type Result<T> = std::result::Result<T, String>;
fn fail(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

fn identify(chain: &str, genesis: &str, isolated: bool) -> Result<&'static str> {
    if chain != "test" {
        return Err("only explicitly identified test chains are supported".into());
    }
    if genesis == TEST_GENESIS && !isolated {
        Ok("Zcash standard testnet")
    } else if genesis == REGTEST_GENESIS && isolated {
        Ok("isolated Ironwood regtest; not standard testnet")
    } else {
        Err("genesis does not match the explicitly requested network".into())
    }
}

fn expected_branch(height: u64, isolated: bool) -> Result<u32> {
    let height = u32::try_from(height).map_err(|_| "block height exceeds u32")?;
    if isolated {
        Ok(BranchId::Nu6_3.into())
    } else {
        Ok(BranchId::for_height(&TEST_NETWORK, BlockHeight::from_u32(height)).into())
    }
}

async fn probe(endpoint: &str, isolated: bool) -> Result<serde_json::Value> {
    if !endpoint.starts_with("https://") {
        return Err("this read-only probe requires HTTPS with certificate validation".into());
    }
    let transport = Endpoint::from_shared(endpoint.to_owned())
        .map_err(|e| fail("endpoint", e))?
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(25))
        .tls_config(ClientTlsConfig::new().with_webpki_roots())
        .map_err(|e| fail("TLS configuration", e))?;
    let mut client = CompactTxStreamerClient::new(
        transport
            .connect()
            .await
            .map_err(|e| fail("TLS connect", e))?,
    );
    let info = client
        .get_lightd_info(Empty {})
        .await
        .map_err(|e| fail("GetLightdInfo", e))?
        .into_inner();
    if info.chain_name != "test" {
        return Err("server reports a non-test chain; refusing further queries".into());
    }
    // TreeState is not defined before Sapling activation, and protobuf height
    // zero is also interpreted as unspecified. Identify genesis from block 1's
    // previous hash instead. Compact hashes use internal little-endian bytes;
    // TreeState's hash string uses display order. Never interchange these APIs.
    let first = client
        .get_block(BlockId {
            height: 1,
            hash: vec![],
        })
        .await
        .map_err(|e| fail("GetBlock(1) for genesis identification", e))?
        .into_inner();
    if first.height != 1 || first.prev_hash.len() != 32 {
        return Err("first compact block has the wrong height or previous hash length".into());
    }
    let mut genesis_bytes = first.prev_hash;
    genesis_bytes.reverse();
    let genesis_hash = hex::encode(genesis_bytes);
    let network = identify(&info.chain_name, &genesis_hash, isolated)?;
    let latest = client
        .get_latest_block(ChainSpec {})
        .await
        .map_err(|e| fail("GetLatestBlock", e))?
        .into_inner();
    let tree = client
        .get_tree_state(BlockId {
            height: latest.height,
            hash: vec![],
        })
        .await
        .map_err(|e| fail("GetTreeState(tip)", e))?
        .into_inner();
    if tree.height != latest.height || tree.network != "test" || latest.hash.len() != 32 {
        return Err("tip TreeState has the wrong height, network or hash length".into());
    }
    let mut latest_hash = latest.hash.clone();
    latest_hash.reverse();
    if hex::encode(latest_hash) != tree.hash.to_ascii_lowercase() {
        return Err("tip hash changed between network queries; retry from a new snapshot".into());
    }
    let branch = u32::from_str_radix(info.consensus_branch_id.trim_start_matches("0x"), 16)
        .map_err(|e| fail("branch ID", e))?;
    // LightdInfo and the later block snapshot may straddle activation. Refuse
    // this snapshot and retry rather than build a transaction on mixed rules.
    if branch != expected_branch(latest.height, isolated)? {
        return Err(format!(
            "server branch ID {branch:08x} does not match pinned network rules {:08x} at height {}",
            expected_branch(latest.height, isolated)?,
            latest.height
        ));
    }
    let compact = client
        .get_block(BlockId {
            height: latest.height,
            hash: vec![],
        })
        .await
        .map_err(|e| fail("GetBlock(tip)", e))?
        .into_inner();
    if compact.height != latest.height || compact.hash != latest.hash {
        return Err("compact block and tree snapshot refer to different tips".into());
    }
    let ironwood_tree_bytes = hex::decode(&tree.ironwood_tree)
        .map_err(|e| fail("Ironwood tree hex", e))?
        .len();
    let ironwood_actions: usize = compact.vtx.iter().map(|tx| tx.ironwood_actions.len()).sum();
    Ok(serde_json::json!({
        "scope": "read-only lightwalletd identification; provider trusted, no consensus validation",
        "endpoint": endpoint,
        "network": network,
        "genesis": genesis_hash,
        "tip_height": latest.height,
        "tip_hash": tree.hash,
        "branch_id": format!("{branch:08x}"),
        "ironwood_active": branch == u32::from(BranchId::Nu6_3),
        "ironwood_tree_encoding_bytes": ironwood_tree_bytes,
        "ironwood_tree_size_reported": compact.chain_metadata.map(|m| m.ironwood_commitment_tree_size),
        "ironwood_actions_in_tip": ironwood_actions,
        "lightwalletd_version": info.version,
        "lightwalletd_git_commit": info.git_commit,
        "lightwallet_protocol_version": info.lightwallet_protocol_version,
        "chain_funding": false,
        "broadcast": false,
    }))
}

#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command, endpoint] if command == "probe-testnet" => probe(endpoint, false).await,
        [command, endpoint] if command == "probe-isolated-regtest" => probe(endpoint, true).await,
        _ => Err(
            "usage: darkcall-wallet-bridge probe-testnet|probe-isolated-regtest HTTPS_ENDPOINT"
                .into(),
        ),
    };
    match result {
        Ok(data) => println!("{}", serde_json::to_string_pretty(&data).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_named_regtest_is_not_standard_testnet() {
        assert!(identify("test", REGTEST_GENESIS, false).is_err());
        assert!(identify("test", TEST_GENESIS, true).is_err());
        assert!(identify("main", TEST_GENESIS, false).is_err());
        assert!(identify("test", TEST_GENESIS, false).is_ok());
        assert!(identify("test", REGTEST_GENESIS, true).is_ok());
    }
    #[test]
    fn upgrade_boundary_and_height_overflow() {
        assert_ne!(
            expected_branch(4_133_999, false).unwrap(),
            u32::from(BranchId::Nu6_3)
        );
        assert_eq!(
            expected_branch(4_134_000, false).unwrap(),
            u32::from(BranchId::Nu6_3)
        );
        assert!(expected_branch(u64::MAX, false).is_err());
    }
}
