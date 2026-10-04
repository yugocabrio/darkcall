//! Synthetic fixture creator and node-side validation of committed REP3 inputs.
//! No real bids, keys or shares should be placed in the Git repository.
use ark_bn254::Fr;
use ark_ff::{Field, PrimeField};
use bincode::Options;
use co_noir_types::{Rep3SharedInput, Rep3Type};
use mpc_core::gadgets::poseidon2::{POSEIDON2_BN254_T4_PARAMS, Poseidon2};
use mpc_core::protocols::rep3;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, str::FromStr};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn hash(input: &[Fr]) -> Fr {
    let permutation = Poseidon2::<Fr, 4, 5>::new(&POSEIDON2_BN254_T4_PARAMS);
    let mut state = [Fr::from(0); 4];
    state[3] = Fr::from(input.len() as u64) * Fr::from(2).pow([64]);
    for chunk in input.chunks_exact(3) {
        for i in 0..3 {
            state[i] += chunk[i];
        }
        permutation.permutation_in_place(&mut state);
    }
    let remainder = input.chunks_exact(3).remainder();
    for (i, value) in remainder.iter().enumerate() {
        state[i] += value;
    }
    permutation.permutation(&state)[0]
}

fn domain(text: &[u8]) -> Fr {
    assert!(text.len() < 31);
    Fr::from_be_bytes_mod_order(text)
}

fn limbs(bytes: &[u8; 32]) -> [Fr; 4] {
    std::array::from_fn(|i| {
        Fr::from(u64::from_be_bytes(
            bytes[i * 8..(i + 1) * 8].try_into().unwrap(),
        ))
    })
}

#[derive(Clone, Serialize, Deserialize)]
struct Book {
    condition_digest: [u8; 32],
    ids: [[u8; 32]; 4],
    active: [bool; 4],
    units: u32,
    reserve: u64,
    budget: u64,
    processing_fee: u64,
    commitments: [String; 4],
    pair_commitments: [[String; 3]; 4],
    root: String,
}

impl Book {
    fn context(&self) -> Fr {
        let mut fields = vec![domain(b"DarkCall/ctx/v1")];
        fields.extend(limbs(&self.condition_digest));
        fields.extend([
            Fr::from(self.units),
            Fr::from(self.reserve),
            Fr::from(self.budget),
            Fr::from(self.processing_fee),
        ]);
        hash(&fields)
    }

    fn leaf(&self, id: &[u8; 32], bid: u64, salt: Fr) -> Fr {
        let mut fields = vec![domain(b"DarkCall/bid/v1"), self.context()];
        fields.extend(limbs(id));
        fields.extend([Fr::from(bid), salt]);
        hash(&fields)
    }

    fn pair(
        &self,
        bidder: usize,
        party: usize,
        b: rep3::Rep3PrimeFieldShare<Fr>,
        s: rep3::Rep3PrimeFieldShare<Fr>,
    ) -> Fr {
        let mut fields = vec![domain(b"DarkCall/share/v1"), self.context()];
        fields.extend(limbs(&self.ids[bidder]));
        fields.extend([Fr::from(party as u64), b.a, b.b, s.a, s.b]);
        hash(&fields)
    }

    fn root(&self) -> Result<Fr> {
        let mut fields = vec![
            domain(b"DarkCall/accepted/v1"),
            self.context(),
            Fr::from(4),
            Fr::from(self.active.iter().filter(|x| **x).count() as u64),
        ];
        fields.extend(self.active.map(|x| Fr::from(u64::from(x))));
        for x in &self.commitments {
            fields.push(Fr::from_str(x).map_err(|_| "invalid commitment")?);
        }
        Ok(hash(&fields))
    }

    fn public_toml(&self) -> Result<String> {
        Ok(format!(
            "condition_digest = {}\nids = {}\nactive = {}\nunits = {}\nreserve = \"{}\"\nbudget = \"{}\"\nprocessing_fee = \"{}\"\ncommitments = {}\nroot = \"{}\"\n",
            serde_json::to_string(&self.condition_digest)?,
            serde_json::to_string(&self.ids)?,
            serde_json::to_string(&self.active)?,
            self.units,
            self.reserve,
            self.budget,
            self.processing_fee,
            serde_json::to_string(&self.commitments)?,
            self.root
        ))
    }

    fn client_public_toml(&self, bidder: usize) -> Result<String> {
        Ok(format!(
            "condition_digest = {}\nid = {}\nunits = {}\nreserve = \"{}\"\nbudget = \"{}\"\nprocessing_fee = \"{}\"\ncommitment = \"{}\"\npair_commitments = {}\n",
            serde_json::to_string(&self.condition_digest)?,
            serde_json::to_string(&self.ids[bidder])?,
            self.units,
            self.reserve,
            self.budget,
            self.processing_fee,
            self.commitments[bidder],
            serde_json::to_string(&self.pair_commitments[bidder])?
        ))
    }

    fn client_fields(&self, bidder: usize) -> Result<Vec<Fr>> {
        let mut fields = Vec::new();
        fields.extend(self.condition_digest.map(Fr::from));
        fields.extend(self.ids[bidder].map(Fr::from));
        fields.extend([
            Fr::from(self.units),
            Fr::from(self.reserve),
            Fr::from(self.budget),
            Fr::from(self.processing_fee),
        ]);
        fields.push(Fr::from_str(&self.commitments[bidder]).map_err(|_| "invalid commitment")?);
        for x in &self.pair_commitments[bidder] {
            fields.push(Fr::from_str(x).map_err(|_| "invalid pair commitment")?);
        }
        Ok(fields)
    }
}

fn read_share(path: &Path) -> Result<Rep3SharedInput<Fr>> {
    if fs::metadata(path)?.len() > 16_384 {
        return Err("share file too large".into());
    }
    let bytes = fs::read(path)?;
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(16_384)
        .reject_trailing_bytes()
        .deserialize(&bytes)
        .map_err(|_| "invalid share encoding".into())
}

fn shared(map: &Rep3SharedInput<Fr>, key: &str) -> Result<rep3::Rep3PrimeFieldShare<Fr>> {
    match map.get(key) {
        Some(Rep3Type::Shared(value)) => Ok(*value),
        _ => Err("missing private share".into()),
    }
}

fn check(
    book: &Book,
    bidder: usize,
    party: usize,
    share_path: &Path,
    public_path: &Path,
) -> Result<()> {
    if bidder >= 4 || party >= 3 {
        return Err("invalid role".into());
    }
    let bytes = fs::read(public_path)?;
    let expected = book.client_fields(bidder)?;
    if bytes.len() != expected.len() * 32 {
        return Err("client statement length mismatch".into());
    }
    for (encoded, field) in bytes.chunks_exact(32).zip(expected) {
        let value = ark_ff::BigInteger::to_bytes_be(&field.into_bigint());
        let mut canonical = [0_u8; 32];
        canonical[32 - value.len()..].copy_from_slice(&value);
        if encoded != canonical {
            return Err("client proof statement mismatch".into());
        }
    }
    let map = read_share(share_path)?;
    if map.len() != 2 {
        return Err("unexpected share entries".into());
    }
    let b = shared(&map, &format!("bid{bidder}"))?;
    let s = shared(&map, &format!("salt{bidder}"))?;
    let target = Fr::from_str(&book.pair_commitments[bidder][party])
        .map_err(|_| "invalid pair commitment")?;
    if book.pair(bidder, party, b, s) != target {
        return Err("delivered share pair does not match proof".into());
    }
    Ok(())
}

fn fixture(out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    let mut book = Book {
        condition_digest: [1; 32],
        ids: std::array::from_fn(|i| {
            let mut a = [0; 32];
            a[31] = i as u8;
            a
        }),
        active: [true; 4],
        units: 3,
        reserve: 20,
        budget: 100,
        processing_fee: 15_000,
        commitments: std::array::from_fn(|_| String::new()),
        pair_commitments: std::array::from_fn(|_| std::array::from_fn(|_| String::new())),
        root: String::new(),
    };
    let mut rng = rand::thread_rng();
    let mut private_client_files = Vec::new();
    for (bidder, bid) in [90_u64, 70, 60, 50].into_iter().enumerate() {
        let salt = <Fr as ark_ff::UniformRand>::rand(&mut rng);
        let bids = rep3::share_field_element(Fr::from(bid), &mut rng);
        let salts = rep3::share_field_element(salt, &mut rng);
        book.commitments[bidder] = book.leaf(&book.ids[bidder], bid, salt).to_string();
        for party in 0..3 {
            book.pair_commitments[bidder][party] = book
                .pair(bidder, party, bids[party], salts[party])
                .to_string();
            let map = BTreeMap::from([
                (format!("bid{bidder}"), Rep3Type::Shared(bids[party])),
                (format!("salt{bidder}"), Rep3Type::Shared(salts[party])),
            ]);
            fs::write(
                out.join(format!("bidder{bidder}.toml.{party}.shared")),
                bincode::serialize(&map)?,
            )?;
        }
        private_client_files.push(format!(
            "bid = \"{bid}\"\nsalt = \"{salt}\"\nbid_shares = {}\nsalt_shares = {}\n",
            serde_json::to_string(&bids.map(|s| s.a.to_string()))?,
            serde_json::to_string(&salts.map(|s| s.a.to_string()))?
        ));
    }
    book.root = book.root()?.to_string();
    fs::write(out.join("book.json"), serde_json::to_vec_pretty(&book)?)?;
    fs::write(out.join("public.toml"), book.public_toml()?)?;
    for (bidder, private) in private_client_files.iter().enumerate() {
        let path = out.join(format!("client{bidder}"));
        fs::create_dir_all(&path)?;
        fs::write(
            path.join("Prover.toml"),
            private.to_owned() + &book.client_public_toml(bidder)?,
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fixture") if args.len()==2 => fixture(Path::new(&args[1]))?,
        Some("check") if args.len()==6 => {
            let book:Book=serde_json::from_slice(&fs::read(&args[1])?)?;
            check(&book,args[2].parse()?,args[3].parse()?,Path::new(&args[4]),Path::new(&args[5]))?;
        },
        Some("tamper") if args.len()==4 => {
            let mut map=read_share(Path::new(&args[1]))?;
            let key=format!("bid{}",args[3].parse::<usize>()?);
            if let Some(Rep3Type::Shared(share))=map.get_mut(&key) {share.a += Fr::from(1);} else {return Err("missing bid".into());}
            fs::write(&args[2],bincode::serialize(&map)?)?;
        },
        _ => return Err("usage: fixture OUT | check BOOK BIDDER PARTY SHARE CLIENT_PUBLIC_INPUT | tamper SHARE OUT BIDDER".into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_noir_standard_sponge() {
        // Hex vector is authoritative; convert it explicitly to avoid decimal transcription.
        let bytes = [
            0x00, 0x22, 0x47, 0xbe, 0x70, 0x14, 0xa5, 0x4d, 0x17, 0x34, 0x2a, 0x7e, 0xf6, 0x77,
            0xf5, 0x8d, 0x28, 0x87, 0x77, 0x80, 0xd2, 0x03, 0x86, 0x03, 0x96, 0x96, 0x7f, 0x5d,
            0x0a, 0x18, 0xd2, 0x59, 0xdb,
        ];
        let from_hex = Fr::from_be_bytes_mod_order(&bytes);
        assert_eq!(hash(&[1, 2, 3, 4, 5].map(Fr::from)), from_hex);
    }
}
