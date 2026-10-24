//! Isolated compatibility experiment. Public RPC is read-only; actual node
//! captures may be scanned locally. No transaction broadcast operation.
mod auction;
mod claims;
mod funded_cancel;
mod funded_prepare;
mod funded_settlement;
mod interop_export;
mod native;
mod network;
mod note_interop;
mod probe;
mod receipt_transport;
mod regtest_admission;
mod regtest_deposits;
mod regtest_transfer;
mod regtest_wallet;
mod testnet;

type Result<T> = std::result::Result<T, String>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("regtest-admission") {
        return regtest_admission::execute(&args[1..]).map_err(|e| e.to_string());
    }
    let result = match args.as_slice() {
        [command] if command == "synthetic-native" => native::gate(),
        [command] if command == "note-interop" => note_interop::gate(),
        [command] if command == "transaction-identity" => auction::transaction_identity(),
        [command, name] if command == "synthetic-native-export" => native::gate_export(name),
        [command, directory] if command == "prepare-auction" => auction::prepare(std::path::Path::new(directory)),
        [command, directory] if command == "finalize-auction" => auction::finalize(std::path::Path::new(directory)),
        [command, directory] if command == "verify-auction" => auction::verify(std::path::Path::new(directory)),
        [command, wallet, capture] if command == "regtest-scan" => regtest_wallet::scan(std::path::Path::new(wallet),std::path::Path::new(capture)),
        [command, wallet, notes, recipient, name] if command == "regtest-transfer" => regtest_transfer::build(std::path::Path::new(wallet),std::path::Path::new(notes),std::path::Path::new(recipient),name),
        [command, wallet, notes, escrow, book, receivers, name] if command == "regtest-deposits" => regtest_deposits::build(std::path::Path::new(wallet),std::path::Path::new(notes),std::path::Path::new(escrow),std::path::Path::new(book),std::path::Path::new(receivers),name),
        [command, escrow, deposit] if command == "regtest-claim-check" => claims::check_deposit(std::path::Path::new(escrow),std::path::Path::new(deposit)),
        [command, directory] if command == "prepare-funded-auction" => funded_prepare::prepare(std::path::Path::new(directory)),
        [command, directory] if command == "funded-cancel-prepare" => funded_cancel::prepare(std::path::Path::new(directory)),
        [command, directory] if command == "funded-cancel-finalize" => funded_cancel::finalize(std::path::Path::new(directory)),
        [command, directory, authority] if command == "funded-certificate-link" => funded_settlement::certificate_link(std::path::Path::new(directory),std::path::Path::new(authority)),
        [command, directory, index, wallet] if command == "funded-receipt-export" => receipt_transport::export_funded(std::path::Path::new(directory),index.parse().map_err(|_| "funded recipient index")?,std::path::Path::new(wallet)),
        [command, directory, index, out] if command == "funded-receipt-open" => receipt_transport::open_funded(std::path::Path::new(directory),index.parse().map_err(|_| "funded recipient index")?,std::path::Path::new(out)),
        [command, directory] if command == "check-funded-prepared" => funded_settlement::finalize(std::path::Path::new(directory),false),
        [command, directory] if command == "finalize-funded-auction" => funded_settlement::finalize(std::path::Path::new(directory),true),
        [command, directory, authority] if command == "verify-funded-auction" => funded_settlement::verify(std::path::Path::new(directory),std::path::Path::new(authority)),
        [command, directory] if command == "receipt-transport" => native::gate_receipts(std::path::Path::new(directory)),
        [command, directory, index, out] if command == "receipt-transport-open" => receipt_transport::open(std::path::Path::new(directory),index.parse().map_err(|_| "recipient index")?,std::path::Path::new(out)),
        [command, directory, index, out] if command == "auction-receipt-open" => receipt_transport::open_auction(std::path::Path::new(directory),index.parse().map_err(|_| "recipient index")?,std::path::Path::new(out)),
        [command, directory] if command == "verify-export" => interop_export::verify(std::path::Path::new(directory),None),
        [command, directory, row] if command == "verify-export" => interop_export::verify(std::path::Path::new(directory),Some(std::path::Path::new(row))),
        [command, endpoint] if command == "probe-testnet" => probe::probe(endpoint).await,
        [command, wallet, endpoint] if command == "testnet-init" => testnet::init(std::path::Path::new(wallet), endpoint).await,
        [command, endpoint] if command == "testnet-status" => testnet::status(endpoint).await,
        [command, endpoint, height] if command == "testnet-tree" => testnet::tree(endpoint, height).await,
        [command, wallet, endpoint] if command == "testnet-capture" => testnet::capture(std::path::Path::new(wallet), endpoint).await,
        [command, endpoint, path] if command == "testnet-send" => testnet::send(endpoint, std::path::Path::new(path)).await,
        [command, endpoint, txid] if command == "testnet-tx" => testnet::transaction(endpoint, txid).await,
        [command, endpoint, notes] if command == "testnet-source-check" => testnet::source_check(endpoint, std::path::Path::new(notes)).await,
        [command, wallet] if command == "testnet-address" => testnet::address(std::path::Path::new(wallet)).map(|a| serde_json::json!({"address":a})),
        [command, path] if command == "verify-old-native" => native::verify_old(path),
        _ => Err("usage: darkcall-nu7-compat regtest-scan WALLET NODE_CAPTURE | regtest-transfer SOURCE_WALLET PRIVATE_NOTES RECIPIENT_WALLET FRESH_RUN_NAME | synthetic-native | note-interop | synthetic-native-export LOCAL_NAME | prepare-auction DIRECTORY | finalize-auction DIRECTORY | verify-auction PUBLIC_DIRECTORY | receipt-transport DIRECTORY | receipt-transport-open RECIPIENT_DIRECTORY INDEX OUT_MEMO | verify-export PUBLIC_DIRECTORY [PUBLIC_ROW] | probe-testnet HTTPS_ENDPOINT | verify-old-native PUBLIC_TRANSACTION | transaction-identity < TRANSACTION".into()),
    }?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}
