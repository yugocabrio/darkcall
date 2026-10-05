//! Isolated compatibility experiment. Read-only public RPC and synthetic notes
//! only. There is deliberately no funding or transaction broadcast operation.
mod auction;
mod interop_export;
mod native;
mod note_interop;
mod probe;
mod receipt_transport;

type Result<T> = std::result::Result<T, String>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command] if command == "synthetic-native" => native::gate(),
        [command] if command == "note-interop" => note_interop::gate(),
        [command, name] if command == "synthetic-native-export" => native::gate_export(name),
        [command, directory] if command == "prepare-auction" => auction::prepare(std::path::Path::new(directory)),
        [command, directory] if command == "finalize-auction" => auction::finalize(std::path::Path::new(directory)),
        [command, directory] if command == "verify-auction" => auction::verify(std::path::Path::new(directory)),
        [command, directory] if command == "receipt-transport" => native::gate_receipts(std::path::Path::new(directory)),
        [command, directory, index, out] if command == "receipt-transport-open" => receipt_transport::open(std::path::Path::new(directory),index.parse().map_err(|_| "recipient index")?,std::path::Path::new(out)),
        [command, directory, index, out] if command == "auction-receipt-open" => receipt_transport::open_auction(std::path::Path::new(directory),index.parse().map_err(|_| "recipient index")?,std::path::Path::new(out)),
        [command, directory] if command == "verify-export" => interop_export::verify(std::path::Path::new(directory),None),
        [command, directory, row] if command == "verify-export" => interop_export::verify(std::path::Path::new(directory),Some(std::path::Path::new(row))),
        [command, endpoint] if command == "probe-testnet" => probe::probe(endpoint).await,
        [command, path] if command == "verify-old-native" => native::verify_old(path),
        _ => Err("usage: darkcall-nu7-compat synthetic-native | note-interop | synthetic-native-export LOCAL_NAME | prepare-auction DIRECTORY | finalize-auction DIRECTORY | verify-auction PUBLIC_DIRECTORY | receipt-transport DIRECTORY | receipt-transport-open RECIPIENT_DIRECTORY INDEX OUT_MEMO | verify-export PUBLIC_DIRECTORY [PUBLIC_ROW] | probe-testnet HTTPS_ENDPOINT | verify-old-native PUBLIC_TRANSACTION".into()),
    }?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}
