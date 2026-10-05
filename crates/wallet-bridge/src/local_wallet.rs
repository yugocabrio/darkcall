//! Private local-regtest wallet loading. A DKG escrow exports only its FVK to
//! scanners/admission; signing shares are loaded separately by the custody gate.
use orchard::keys::{FullViewingKey, SpendingKey};
use std::{fs, io::Read, path::Path};

pub const DKG_READY: &[u8] = b"DarkCall/local-DKG/v1/ready\n";

pub fn read_private(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".local")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(root) {
        return Err("private wallet path outside ignored local tree".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err("invalid private wallet file shape".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.permissions().mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err("wallet file permissions/link count".into());
        }
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

pub fn viewing_key(directory: &Path) -> Result<FullViewingKey, String> {
    let sk = directory.join("private-spending-key.bin");
    let fvk = directory.join("private-full-viewing-key.bin");
    // Do not fall back to a spending key when DKG/view-only files are incomplete
    // or malformed. Both formats at once are ambiguous and always rejected.
    match (
        sk.try_exists().map_err(|e| e.to_string())?,
        fvk.try_exists().map_err(|e| e.to_string())?,
    ) {
        (true, false) => {
            if ["dkg-ready", "private-frost-public-package.bin"]
                .iter()
                .try_fold(false, |found, name| {
                    directory
                        .join(name)
                        .try_exists()
                        .map(|exists| found || exists)
                })
                .map_err(|e| e.to_string())?
            {
                return Err("DKG escrow unexpectedly contains a master spending key".into());
            }
            let bytes: [u8; 32] = read_private(&sk, 32)?
                .try_into()
                .map_err(|_| "spending key length")?;
            let key = SpendingKey::from_bytes(bytes)
                .into_option()
                .ok_or("invalid spending key")?;
            Ok(FullViewingKey::from(&key))
        }
        (false, true) => {
            if read_private(&directory.join("dkg-ready"), 64)?.as_slice() != DKG_READY {
                return Err("DKG setup is not durably complete".into());
            }
            let bytes: [u8; 96] = read_private(&fvk, 96)?
                .try_into()
                .map_err(|_| "viewing key length")?;
            FullViewingKey::from_bytes(&bytes).ok_or("invalid full viewing key".into())
        }
        _ => Err("expected exactly one wallet key format".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn write(path: &Path, bytes: &[u8]) {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path).unwrap().write_all(bytes).unwrap();
    }
    #[test]
    fn view_only_requires_complete_setup_and_rejects_master_aliases() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".local");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = root.join(format!("viewing-key-test-{}-{stamp}", std::process::id()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory).unwrap();
        let sk = SpendingKey::from_bytes([7; 32]).unwrap();
        let expected = FullViewingKey::from(&sk);
        write(
            &directory.join("private-full-viewing-key.bin"),
            &expected.to_bytes(),
        );
        assert!(viewing_key(&directory).is_err());
        write(&directory.join("dkg-ready"), DKG_READY);
        assert_eq!(
            viewing_key(&directory).unwrap().to_bytes(),
            expected.to_bytes()
        );
        write(&directory.join("private-spending-key.bin"), sk.to_bytes());
        assert!(viewing_key(&directory).is_err());
        // Only this freshly-created test directory is removed, never real wallets.
        fs::remove_dir_all(directory).unwrap();
    }
}
