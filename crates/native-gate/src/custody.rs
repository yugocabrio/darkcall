//! Local three-participant FROST DKG. No escrow master ASK or SpendingKey is
//! generated/reconstructed. All shares still live on one host in this test gate;
//! authenticated independent participant transport is a separate requirement.
use super::*;
use crate::regtest_spend::{read_private, write_private};
use darkcall_wallet_bridge::local_wallet::DKG_READY;
use std::{fs, path::Path};

fn identifiers() -> [frost::Identifier; 3] {
    [1u16, 2, 3].map(|i| i.try_into().expect("nonzero fixed participant"))
}

fn generate(rng: &mut OsRng) -> Result<(FullViewingKey, ThresholdKeys)> {
    let ids = identifiers();
    let mut secret1 = BTreeMap::new();
    let mut received1: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
    for id in ids {
        let (secret, package) =
            frost::keys::dkg::part1(id, 3, 2, *rng).map_err(|e| error("DKG part1", e))?;
        secret1.insert(id, secret);
        for receiver in ids.into_iter().filter(|other| *other != id) {
            received1
                .entry(receiver)
                .or_default()
                .insert(id, package.clone());
        }
    }
    let mut secret2 = BTreeMap::new();
    let mut received2: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
    for id in ids {
        let (secret, packages) = frost::keys::dkg::part2(
            secret1.remove(&id).ok_or("missing DKG secret")?,
            &received1[&id],
        )
        .map_err(|e| error("DKG part2", e))?;
        secret2.insert(id, secret);
        for (receiver, package) in packages {
            received2.entry(receiver).or_default().insert(id, package);
        }
    }
    let mut packages = BTreeMap::new();
    let mut public = None;
    for id in ids {
        // The pinned RedPallas wrapper normalizes *both* shares and group public
        // key to even Y. Do not independently negate only the public key.
        let (key, group) = frost::keys::dkg::part3(&secret2[&id], &received1[&id], &received2[&id])
            .map_err(|e| error("DKG part3", e))?;
        if let Some(expected) = &public
            && expected != &group
        {
            return Err("DKG participant group keys disagree".into());
        }
        public = Some(group);
        packages.insert(id, key);
    }
    let keys = ThresholdKeys {
        packages,
        public: public.ok_or("no DKG public key")?,
    };
    let group = keys
        .public
        .verifying_key()
        .serialize()
        .map_err(|e| error("DKG public bytes", e))?;
    if group.len() != 32 || group[31] & 0x80 != 0 {
        return Err("DKG group key violates native even-Y encoding".into());
    }
    // Independent random viewing material. It provides scanning capability,
    // never a scalar corresponding to the DKG spend authorization group key.
    let fvk = loop {
        let mut bytes = [0u8; 96];
        bytes[..32].copy_from_slice(&group);
        bytes[32..64].copy_from_slice(&pallas::Base::random(&mut *rng).to_repr());
        bytes[64..].copy_from_slice(&pallas::Scalar::random(&mut *rng).to_repr());
        if let Some(fvk) = FullViewingKey::from_bytes(&bytes) {
            break fvk;
        }
    };
    validate(&fvk, &keys)?;
    Ok((fvk, keys))
}

fn validate(fvk: &FullViewingKey, keys: &ThresholdKeys) -> Result<()> {
    let group = keys
        .public
        .verifying_key()
        .serialize()
        .map_err(|e| error("group key", e))?;
    if fvk.to_bytes()[..32] != group
        || keys.packages.len() != 3
        || keys.public.verifying_shares().len() != 3
    {
        return Err("escrow viewing/group/participant mismatch".into());
    }
    for id in identifiers() {
        let key = keys.packages.get(&id).ok_or("missing DKG operator")?;
        if key.identifier() != &id
            || key.min_signers() != &2
            || key.verifying_key() != keys.public.verifying_key()
            || keys.public.verifying_shares().get(&id) != Some(key.verifying_share())
            || frost::keys::VerifyingShare::from(*key.signing_share()) != *key.verifying_share()
        {
            return Err("invalid DKG key package/group/share/threshold".into());
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|e| error("new DKG directory", e))?;
    fs::File::open(path.parent().ok_or("DKG parent")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| error("DKG parent sync", e))
}

pub fn init(name: &str) -> Result<serde_json::Value> {
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("fresh simple ignored DKG directory name required".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".local")
        .canonicalize()
        .map_err(|e| error("local tree", e))?;
    let directory = root.join(name);
    let (fvk, keys) = generate(&mut OsRng)?;
    private_directory(&directory)?;
    write_private(
        &directory.join("private-full-viewing-key.bin"),
        &fvk.to_bytes(),
    )?;
    write_private(
        &directory.join("private-frost-public-package.bin"),
        &keys
            .public
            .serialize()
            .map_err(|e| error("public key package encoding", e))?,
    )?;
    for (i, id) in identifiers().into_iter().enumerate() {
        let operator = directory.join(format!("operator-{i}"));
        private_directory(&operator)?;
        write_private(
            &operator.join("private-frost-key-package.bin"),
            &keys.packages[&id]
                .serialize()
                .map_err(|e| error("private key package encoding", e))?,
        )?;
    }
    // Publish last, only after every file and its containing directory are synced.
    write_private(&directory.join("dkg-ready"), DKG_READY)?;
    let (_, loaded) = load(&directory)?;
    if loaded.public != keys.public {
        return Err("persisted DKG public key differs".into());
    }
    Ok(
        serde_json::json!({"scope":"local three-participant DKG escrow initialization; all shares on one host","threshold":"2-of-3","escrow_master_spending_key_generated":false,"escrow_full_ask_reconstructed":false,"native_fvk_valid":true,"persisted_key_packages_reloaded":true,"independent_participant_transport":false,"directory":name,"private_key_material_logged":false}),
    )
}

pub fn load(directory: &Path) -> Result<(FullViewingKey, ThresholdKeys)> {
    if directory
        .join("private-spending-key.bin")
        .try_exists()
        .map_err(|e| error("master key existence", e))?
    {
        return Err("DKG escrow must not contain a master spending key".into());
    }
    let fvk = darkcall_wallet_bridge::local_wallet::viewing_key(directory)?;
    let public = frost::keys::PublicKeyPackage::deserialize(&read_private(
        &directory.join("private-frost-public-package.bin"),
        16384,
    )?)
    .map_err(|e| error("DKG public package", e))?;
    let mut packages = BTreeMap::new();
    for (i, id) in identifiers().into_iter().enumerate() {
        let key = frost::keys::KeyPackage::deserialize(&read_private(
            &directory.join(format!("operator-{i}/private-frost-key-package.bin")),
            16384,
        )?)
        .map_err(|e| error("DKG operator key", e))?;
        packages.insert(id, key);
    }
    let keys = ThresholdKeys { packages, public };
    validate(&fvk, &keys)?;
    Ok((fvk, keys))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dkg_native_key_and_every_two_party_quorum() {
        let (fvk, keys) = generate(&mut OsRng).unwrap();
        let ids = identifiers();
        let alpha = pallas::Scalar::random(OsRng);
        let message = [42; 32];
        let mut raw = [0; 32];
        raw.copy_from_slice(&fvk.to_bytes()[..32]);
        let vk = orchard::primitives::redpallas::VerificationKey::<SpendAuth>::try_from(raw)
            .unwrap()
            .randomize(&alpha);
        for omitted in ids {
            let subset = ThresholdKeys {
                packages: keys
                    .packages
                    .iter()
                    .filter(|(id, _)| **id != omitted)
                    .map(|(id, k)| (*id, k.clone()))
                    .collect(),
                public: keys.public.clone(),
            };
            let signature = threshold_signature(&subset, &alpha, &message, &mut OsRng).unwrap();
            vk.verify(&message, &signature).unwrap();
            assert!(vk.verify(&[43; 32], &signature).is_err());
        }
        let mut changed = keys;
        changed
            .packages
            .insert(ids[0], changed.packages[&ids[1]].clone());
        assert!(validate(&fvk, &changed).is_err());
    }
}
