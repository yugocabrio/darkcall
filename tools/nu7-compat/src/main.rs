//! Isolated compatibility experiment. Read-only public RPC and synthetic notes
//! only. There is deliberately no funding or transaction broadcast operation.
mod native;
mod note_interop;
mod probe;

type Result<T> = std::result::Result<T, String>;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [command] if command == "synthetic-native" => native::gate(),
        [command] if command == "note-interop" => note_interop::gate(),
        [command, endpoint] if command == "probe-testnet" => probe::probe(endpoint).await,
        [command, path] if command == "verify-old-native" => native::verify_old(path),
        _ => Err("usage: darkcall-nu7-compat synthetic-native | note-interop | probe-testnet HTTPS_ENDPOINT | verify-old-native PUBLIC_TRANSACTION".into()),
    }?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    Ok(())
}
