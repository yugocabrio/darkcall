//! Public Zcash testnet through one lightwalletd endpoint (HTTPS, validated TLS).
//! Every connection re-checks the chain name, the testnet genesis and the NU7
//! branch at the tip before any other query. The provider is trusted for chain
//! data, as the local Zebra node is on regtest; the network still validates every
//! transaction it accepts. Keys stay in the ignored .local tree and are never
//! sent anywhere. Testnet only: mainnet is refused by construction.
use crate::{Result, auction, interop_export::write, regtest_wallet};
use orchard::keys::{Scope, SpendingKey};
use std::{fs, io::Cursor, os::unix::fs::PermissionsExt, path::Path, time::Duration};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use zcash_address::unified::{Address, Encoding, Receiver, Revision, Uitem};
use zcash_client_backend::proto::service::{
    BlockId, BlockRange, ChainSpec, Empty, RawTransaction, TxFilter,
    compact_tx_streamer_client::CompactTxStreamerClient,
};
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId, NetworkType, TEST_NETWORK};

pub const TEST_GENESIS: &str = "05a60a92d99d85997cce3b87616c089f6124d7342af37106edc76126334a2c38";
pub const NU7_HEIGHT: u32 = 4_465_026;
type Client = CompactTxStreamerClient<Channel>;

fn fail(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

/// Display-order hex of a compact (internal byte order) 32-byte hash.
pub(crate) fn display(hash: &[u8]) -> Result<String> {
    if hash.len() != 32 {
        return Err("compact hash length".into());
    }
    let mut bytes = hash.to_vec();
    bytes.reverse();
    Ok(hex::encode(bytes))
}

/// Connects and proves, before anything else, that this is standard testnet with
/// NU7 active at the tip. Returns the client and the tip height.
pub(crate) async fn connect(endpoint: &str) -> Result<(Client, u32)> {
    if !endpoint.starts_with("https://") {
        return Err("testnet access requires HTTPS with certificate validation".into());
    }
    let transport = Endpoint::from_shared(endpoint.to_owned())
        .map_err(|e| fail("endpoint", e))?
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .tls_config(ClientTlsConfig::new().with_webpki_roots())
        .map_err(|e| fail("TLS configuration", e))?;
    let mut client =
        CompactTxStreamerClient::new(transport.connect().await.map_err(|e| fail("TLS connect", e))?);
    let info = client
        .get_lightd_info(Empty {})
        .await
        .map_err(|e| fail("GetLightdInfo", e))?
        .into_inner();
    if info.chain_name != "test" {
        return Err("server reports a non-test chain; refusing further queries".into());
    }
    let first = client
        .get_block(BlockId { height: 1, hash: vec![] })
        .await
        .map_err(|e| fail("GetBlock(1)", e))?
        .into_inner();
    if first.height != 1 || display(&first.prev_hash)? != TEST_GENESIS {
        return Err("genesis is not standard testnet".into());
    }
    let tip = client
        .get_latest_block(ChainSpec {})
        .await
        .map_err(|e| fail("GetLatestBlock", e))?
        .into_inner();
    let height = u32::try_from(tip.height).map_err(|_| "tip height range")?;
    let branch = u32::from_str_radix(info.consensus_branch_id.trim_start_matches("0x"), 16)
        .map_err(|e| fail("branch ID", e))?;
    if height < NU7_HEIGHT
        || branch != u32::from(BranchId::Nu7)
        || BranchId::for_height(&TEST_NETWORK, BlockHeight::from_u32(height)) != BranchId::Nu7
    {
        return Err("testnet tip is not under the pinned NU7 rules".into());
    }
    Ok((client, height))
}

/// Native frontier snapshot in the same shape the NU7 scanner reads.
pub(crate) async fn snapshot(client: &mut Client, height: u32) -> Result<serde_json::Value> {
    let tree = client
        .get_tree_state(BlockId { height: u64::from(height), hash: vec![] })
        .await
        .map_err(|e| fail("GetTreeState", e))?
        .into_inner();
    let block = client
        .get_block(BlockId { height: u64::from(height), hash: vec![] })
        .await
        .map_err(|e| fail("GetBlock", e))?
        .into_inner();
    if tree.network != "test"
        || tree.height != u64::from(height)
        || display(&block.hash)? != tree.hash.to_ascii_lowercase()
    {
        return Err("tree state and block disagree; retry from a new snapshot".into());
    }
    Ok(serde_json::json!({"height":height,"hash":tree.hash.to_ascii_lowercase(),"ironwood_tree":tree.ironwood_tree}))
}

pub(crate) fn address(wallet: &Path) -> Result<String> {
    let fvk = regtest_wallet::viewing_key(wallet)?;
    let receiver = fvk.address_at(0u32, Scope::External).to_raw_address_bytes();
    Ok(Address::try_from_items(Revision::R0, vec![Uitem::Data(Receiver::Orchard(receiver))])
        .map_err(|e| fail("unified address", e))?
        .encode(&NetworkType::Test))
}

/// Fresh ordinary bank wallet for public testnet. The birthday snapshot is taken
/// before the address exists anywhere else, so every later note is scannable.
pub async fn init(wallet: &Path, endpoint: &str) -> Result<serde_json::Value> {
    let local = auction::directory(wallet.parent().ok_or("wallet parent")?)?;
    let name = wallet.file_name().and_then(|n| n.to_str()).ok_or("wallet name")?;
    if !name.starts_with("testnet-") || wallet.exists() {
        return Err("use a fresh .local/testnet-* wallet directory; existing wallets are preserved".into());
    }
    let (mut client, height) = connect(endpoint).await?;
    let birthday = snapshot(&mut client, height).await?;
    let directory = local.join(name);
    fs::create_dir(&directory).map_err(|_| "wallet directory")?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).map_err(|_| "wallet mode")?;
    let mut rng = rand_old::rngs::OsRng;
    let key = loop {
        let mut bytes = [0u8; 32];
        rand_old::RngCore::fill_bytes(&mut rng, &mut bytes);
        if let Some(key) = SpendingKey::from_bytes(bytes).into_option() {
            break key;
        }
    };
    write(&directory.join("private-spending-key.bin"), key.to_bytes())?;
    write(&directory.join("nu7-birthday.json"), birthday.to_string().as_bytes())?;
    let address = address(&directory)?;
    let network = serde_json::json!({"network":"Zcash standard testnet","genesis":TEST_GENESIS,
        "branch":"77190ad9","nu7_height":NU7_HEIGHT,"endpoint":endpoint,"birthday_height":height,
        "address":address});
    write(&directory.join("testnet-network.json"), network.to_string().as_bytes())?;
    Ok(serde_json::json!({"scope":"public testnet bank wallet; key stays in .local","address":address,
        "birthday_height":height,"endpoint":endpoint,"secret_key_logged":false}))
}

fn json_file(path: &Path, limit: u64) -> Result<serde_json::Value> {
    serde_json::from_slice(&auction::read(path, limit)?).map_err(|_| "JSON schema".into())
}

fn bytes32(value: &serde_json::Value) -> Result<[u8; 32]> {
    serde_json::from_value(value.clone()).map_err(|_| "32-byte field".into())
}

fn parse_nu7(bytes: &[u8]) -> Result<Transaction> {
    let mut cursor = Cursor::new(bytes);
    let tx = Transaction::read(&mut cursor, BranchId::Nu7).map_err(|_| "NU7 transaction parse")?;
    if cursor.position() != bytes.len() as u64 || tx.consensus_branch_id() != BranchId::Nu7 {
        return Err("NU7 transaction trailing bytes or branch".into());
    }
    Ok(tx)
}

async fn compact(client: &mut Client, height: u32) -> Result<zcash_client_backend::proto::compact_formats::CompactBlock> {
    let block = client
        .get_block(BlockId { height: u64::from(height), hash: vec![] })
        .await
        .map_err(|e| fail("GetBlock", e))?
        .into_inner();
    if block.height != u64::from(height) {
        return Err("provider returned another block height".into());
    }
    Ok(block)
}

/// Tip identity, in the shape the regtest drivers read from their node.
pub async fn status(endpoint: &str) -> Result<serde_json::Value> {
    let (mut client, tip) = connect(endpoint).await?;
    let block = compact(&mut client, tip).await?;
    Ok(serde_json::json!({"scope":"standard public testnet through a trusted lightwalletd provider",
        "genesis":TEST_GENESIS,"branch":"77190ad9","nu7_height":NU7_HEIGHT,"blocks":tip,
        "tip":display(&block.hash)?,"endpoint":endpoint}))
}

/// Frontier snapshot at HEIGHT, or at the tip for "tip".
pub async fn tree(endpoint: &str, height: &str) -> Result<serde_json::Value> {
    let (mut client, tip) = connect(endpoint).await?;
    let height = if height == "tip" { tip } else { height.parse().map_err(|_| "height")? };
    if height > tip || height < NU7_HEIGHT {
        return Err("snapshot height outside NU7 testnet history".into());
    }
    snapshot(&mut client, height).await
}

/// Captures a wallet from its birthday to the tip (or to END) in the regtest
/// capture format and runs the same scanner. An earlier END gives notes with
/// witnesses at that block's anchor, which stays valid for spending. Full transactions are fetched only for compact
/// transactions with Ironwood actions; the scanner then checks every block's
/// Ironwood tree size and the final frontier root against the provider.
pub async fn capture(wallet: &Path, endpoint: &str, end: Option<u32>) -> Result<serde_json::Value> {
    let directory = auction::directory(wallet)?;
    let initial = json_file(&directory.join("nu7-birthday.json"), 100_000)?;
    let start = initial["height"].as_u64().and_then(|h| u32::try_from(h).ok()).ok_or("birthday height")?;
    let (mut client, chain_tip) = connect(endpoint).await?;
    let tip = end.unwrap_or(chain_tip);
    if tip > chain_tip || tip < start || tip - start > 20_000 {
        return Err("wallet birthday outside the bounded testnet capture window".into());
    }
    let final_tree = snapshot(&mut client, tip).await?;
    let mut blocks = Vec::new();
    if tip > start {
        let mut stream = client
            .get_block_range(BlockRange {
                start: Some(BlockId { height: u64::from(start) + 1, hash: vec![] }),
                end: Some(BlockId { height: u64::from(tip), hash: vec![] }),
                pool_types: vec![],
            })
            .await
            .map_err(|e| fail("GetBlockRange", e))?
            .into_inner();
        let mut compacts = Vec::new();
        while let Some(block) = stream.message().await.map_err(|e| fail("compact block stream", e))? {
            compacts.push(block);
        }
        for block in compacts {
            let mut transactions = Vec::new();
            for tx in block.vtx.iter().filter(|t| !t.ironwood_actions.is_empty()) {
                let raw = client
                    .get_transaction(TxFilter { block: None, index: 0, hash: tx.txid.clone() })
                    .await
                    .map_err(|e| fail("GetTransaction", e))?
                    .into_inner();
                // The full transaction must be exactly the one the block listed.
                if parse_nu7(&raw.data)?.txid().as_ref() != tx.txid.as_slice() {
                    return Err("provider returned another transaction".into());
                }
                transactions.push(hex::encode(&raw.data));
            }
            let size = block.chain_metadata.as_ref().ok_or("compact chain metadata")?.ironwood_commitment_tree_size;
            blocks.push(serde_json::json!({"height":block.height,"hash":display(&block.hash)?,
                "prev_hash":display(&block.prev_hash)?,"time":block.time,"ironwood_size":size,
                "transactions":transactions}));
        }
    }
    if blocks.len() as u64 != u64::from(tip - start) {
        return Err("compact block range incomplete".into());
    }
    let capture = serde_json::json!({"genesis":TEST_GENESIS,"branch":"77190ad9","nu7_height":NU7_HEIGHT,
        "initial":initial,"final_tree":final_tree,"blocks":blocks});
    let path = directory.join(format!("testnet-capture-{tip}.json"));
    if path.exists() {
        if json_file(&path, 100_000_000)? != capture {
            return Err("existing capture at this tip differs; preserve it".into());
        }
    } else {
        write(&path, capture.to_string().as_bytes())?;
    }
    let scan = regtest_wallet::scan(&directory, &path)?;
    Ok(serde_json::json!({"scan":scan,"capture":path,"notes":directory.join(format!("private-nu7-notes-{tip}.json")),
        "tip_height":tip,"tip_hash":final_tree["hash"]}))
}

/// Broadcasts one NU7 transaction through the provider.
pub async fn send(endpoint: &str, path: &Path) -> Result<serde_json::Value> {
    auction::directory(path.parent().ok_or("transaction parent")?)?;
    let bytes = auction::read(path, 1_000_000)?;
    let txid = parse_nu7(&bytes)?.txid().to_string();
    let (mut client, tip) = connect(endpoint).await?;
    let response = client
        .send_transaction(RawTransaction { data: bytes, height: 0 })
        .await
        .map_err(|e| fail("SendTransaction", e))?
        .into_inner();
    if response.error_code != 0 {
        return Err(format!("network rejected the transaction ({}): {}", response.error_code, response.error_message));
    }
    let echoed = response.error_message.trim().trim_matches('"');
    if !echoed.is_empty() && echoed != txid {
        return Err("provider acknowledged another transaction ID".into());
    }
    Ok(serde_json::json!({"txid":txid,"submitted_at_tip":tip,"public_testnet":true}))
}

/// Where a transaction is: unknown, in the mempool, or mined in the best chain.
pub async fn transaction(endpoint: &str, txid: &str) -> Result<serde_json::Value> {
    let mut id = hex::decode(txid).map_err(|_| "txid hex")?;
    if id.len() != 32 {
        return Err("txid length".into());
    }
    id.reverse();
    let (mut client, tip) = connect(endpoint).await?;
    let raw = match client.get_transaction(TxFilter { block: None, index: 0, hash: id.clone() }).await {
        Ok(raw) => raw.into_inner(),
        Err(_) => return Ok(serde_json::json!({"txid":txid,"found":false,"tip":tip})),
    };
    if parse_nu7(&raw.data)?.txid().as_ref() != id.as_slice() {
        return Err("provider returned another transaction".into());
    }
    let Ok(height) = u32::try_from(raw.height) else {
        return Ok(serde_json::json!({"txid":txid,"found":true,"mined":false,"tip":tip}));
    };
    if height == 0 || height > tip {
        return Ok(serde_json::json!({"txid":txid,"found":true,"mined":false,"tip":tip}));
    }
    let block = compact(&mut client, height).await?;
    let in_block = block.vtx.iter().any(|t| t.txid == id);
    Ok(serde_json::json!({"txid":txid,"found":true,"mined":in_block,"in_active_chain":in_block,
        "height":height,"block_hash":display(&block.hash)?,"confirmations":tip - height + 1,"tip":tip}))
}

/// Provenance of scanned notes before spending them: the scan's tip is still in
/// the best chain, the anchor is the provider's frontier root there, and each
/// note's commitment sits in its mined block with at least two confirmations.
pub async fn source_check(endpoint: &str, notes_path: &Path) -> Result<serde_json::Value> {
    auction::directory(notes_path.parent().ok_or("notes parent")?)?;
    let notes = json_file(notes_path, 1_000_000)?;
    let chain = crate::network::Chain::checked(
        notes["genesis"].as_str().unwrap_or_default(),
        notes["branch"].as_str().unwrap_or_default(),
        notes["nu7_height"].as_u64().and_then(|h| u32::try_from(h).ok()).unwrap_or(0),
    )?;
    if chain != crate::network::Chain::Testnet || notes["scope"] != chain.openings_scope() {
        return Err("notes are not public testnet openings".into());
    }
    let snapshot_height = notes["tip_height"].as_u64().and_then(|h| u32::try_from(h).ok()).ok_or("notes tip")?;
    let (mut client, tip) = connect(endpoint).await?;
    let block = compact(&mut client, snapshot_height).await?;
    if Some(display(&block.hash)?.as_str()) != notes["tip_hash"].as_str() {
        return Err("note snapshot is no longer on the best chain; rescan".into());
    }
    let state = snapshot(&mut client, snapshot_height).await?;
    let frontier = crate::probe::frontier(state["ironwood_tree"].as_str().unwrap_or_default())?;
    if frontier.root().to_bytes() != bytes32(&notes["anchor"])? {
        return Err("note anchor differs from the provider frontier".into());
    }
    let list = notes["notes"].as_array().ok_or("notes list")?;
    for note in list {
        let mined = note["mined_height"].as_u64().and_then(|h| u32::try_from(h).ok()).ok_or("mined height")?;
        let block = compact(&mut client, mined).await?;
        let txid = bytes32(&note["txid"])?;
        let index = note["action_index"].as_u64().ok_or("action index")? as usize;
        let action = block
            .vtx
            .iter()
            .find(|t| t.txid == txid)
            .and_then(|t| t.ironwood_actions.get(index))
            .ok_or("note transaction or action is not in its mined block")?;
        if block.hash != bytes32(&note["mined_block"])? || action.cmx != bytes32(&note["cmx"])? {
            return Err("note block or commitment differs from the provider".into());
        }
        if tip < mined + 1 {
            return Err("note has fewer than two confirmations".into());
        }
    }
    Ok(serde_json::json!({"notes":list.len(),"snapshot_height":snapshot_height,"tip":tip,
        "snapshot_canonical":true,"anchor_matches":true,"public_testnet":true}))
}
