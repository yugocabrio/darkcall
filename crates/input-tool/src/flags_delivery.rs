//! Explicit independent masked winner bits, including zero-price winners.
//! The caller verifies delivery_private4 or delivery_flags4 before opening.
use super::*;
use std::io::Read;

fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > limit {
        return Err("expected bounded regular delivery file".into());
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("delivery file exceeds bound".into());
    }
    Ok(bytes)
}

fn blinded_commitment(book: &Book, i: usize, mask: Fr, blind: Fr, winner: bool) -> Result<Fr> {
    let tag: &[u8] = if winner {
        b"DarkCall/winner-mask/v2"
    } else {
        b"DarkCall/fill-mask/v2"
    };
    let mut fields = vec![domain(tag), book.context(), canonical_field(&book.root)?];
    fields.extend(limbs(&book.ids[i]));
    fields.extend([mask, blind]);
    Ok(hash(&fields))
}
fn winner_commitment(book: &Book, i: usize, mask: Fr, blind: Fr) -> Result<Fr> {
    blinded_commitment(book, i, mask, blind, true)
}

pub(super) fn fixture(book: &Book, out: &Path, flags: bool) -> Result<()> {
    book.validate_four_active()?;
    fs::create_dir(out)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(out, fs::Permissions::from_mode(0o700))?;
    }
    let mut rng = rand::thread_rng();
    let mut operator = String::new();
    let mut public = book
        .public_toml()?
        .lines()
        .filter(|line| !line.starts_with("active = "))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    for winner in [false, true].into_iter().take(if flags { 2 } else { 1 }) {
        let pads: [[Fr; 2]; 4] = std::array::from_fn(|_| {
            std::array::from_fn(|_| <Fr as ark_ff::UniformRand>::rand(&mut rng))
        });
        let amounts = pads.map(|p| p[0].to_string());
        let blinds = pads.map(|p| p[1].to_string());
        let commitments = (0..4)
            .map(|i| {
                blinded_commitment(book, i, pads[i][0], pads[i][1], winner).map(|v| v.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        let prefix = if winner { "winner" } else { "output" };
        operator += &format!(
            "{prefix}_masks = {}\n{prefix}_blinds = {}\n",
            serde_json::to_string(&amounts)?,
            serde_json::to_string(&blinds)?
        );
        let key = if winner {
            "winner_mask_commitments"
        } else {
            "mask_commitments"
        };
        public += &format!("{key} = {}\n", serde_json::to_string(&commitments)?);
        let filename = if winner {
            "winner-masks.json"
        } else {
            "masks.json"
        };
        write_private(
            &out.join(filename),
            &serde_json::to_vec(&pads.map(|p| p.map(|v| v.to_string())))?,
        )?;
    }
    write_private(&out.join("operator.toml"), operator.as_bytes())?;
    write_private(&out.join("public.toml"), public.as_bytes())
}

fn masks(path: &Path) -> Result<Vec<[Fr; 2]>> {
    if fs::metadata(path)?.len() > 4096 {
        return Err("mask file too large".into());
    }
    let pairs: [[String; 2]; 4] = serde_json::from_slice(&bounded(path, 4096)?)?;
    pairs
        .iter()
        .map(|p| Ok([canonical_field(&p[0])?, canonical_field(&p[1])?]))
        .collect()
}

/// A private per-ticket opening of two fields in a publicly verifiable proof.
/// This is not an operator signature, an admission receipt or a chain receipt.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TicketReceipt {
    format: u32,
    rule: String,
    ticket: [u8; 32],
    accepted_root: String,
    refund_pad: [String; 2],
    winner_pad: [String; 2],
}
const TICKET_RULE: &str = "private-ticket-opening-delivery-flags-v2";
// ZIP 302 arbitrary memo marker followed by a fixed, versioned binary format.
// Ticket is raw bytes; root and the four pad fields are canonical big-endian
// BN254 scalars. All remaining memo bytes must be zero, never extra disclosures.
const MEMO_HEADER: &[u8; 9] = b"\xffDCTKT001";
const MEMO_USED: usize = 201;

impl TicketReceipt {
    fn memo(&self) -> Result<[u8; 512]> {
        if self.format != 1 || self.rule != TICKET_RULE {
            return Err("ticket memo version/domain mismatch".into());
        }
        let mut bytes = [0; 512];
        bytes[..9].copy_from_slice(MEMO_HEADER);
        bytes[9..41].copy_from_slice(&self.ticket);
        let values = std::iter::once(&self.accepted_root)
            .chain(&self.refund_pad)
            .chain(&self.winner_pad);
        for (chunk, value) in bytes[41..MEMO_USED].chunks_exact_mut(32).zip(values) {
            chunk.copy_from_slice(&encoded_field(canonical_field(value)?));
        }
        Ok(bytes)
    }

    fn from_memo(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 512
            || bytes[..9] != *MEMO_HEADER
            || bytes[MEMO_USED..].iter().any(|v| *v != 0)
        {
            return Err("ticket memo length/version/padding mismatch".into());
        }
        let fields = bytes[41..MEMO_USED]
            .chunks_exact(32)
            .map(|encoded| {
                let field = Fr::from_be_bytes_mod_order(encoded);
                if encoded_field(field) != encoded {
                    return Err("noncanonical ticket memo field".into());
                }
                Ok(field.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            format: 1,
            rule: TICKET_RULE.into(),
            ticket: bytes[9..41].try_into()?,
            accepted_root: fields[0].clone(),
            refund_pad: [fields[1].clone(), fields[2].clone()],
            winner_pad: [fields[3].clone(), fields[4].clone()],
        })
    }
}

fn ticket_statement(book: &Book, path: &Path) -> Result<Vec<Fr>> {
    let bytes = bounded(path, 187 * 32)?;
    if bytes.len() != 187 * 32 {
        return Err("ticket receipt requires exact v2 winner statement".into());
    }
    let fields = bytes
        .chunks_exact(32)
        .map(|b| {
            let value = Fr::from_be_bytes_mod_order(b);
            if encoded_field(value) != b {
                return Err("noncanonical ticket statement field".into());
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>>>()?;
    if fields[..169] != book_fields(book)? {
        return Err("ticket receipt proof differs from expected accepted book".into());
    }
    Ok(fields)
}
fn ticket_opening(
    book: &Book,
    receipt: &TicketReceipt,
    fields: &[Fr],
) -> Result<serde_json::Value> {
    if receipt.format != 1 || receipt.rule != TICKET_RULE || receipt.accepted_root != book.root {
        return Err("ticket receipt version/domain/root mismatch".into());
    }
    let i = book
        .ids
        .iter()
        .position(|id| *id == receipt.ticket)
        .ok_or("ticket not in accepted book")?;
    let refund = receipt
        .refund_pad
        .each_ref()
        .map(|s| canonical_field(s))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    let winner = receipt
        .winner_pad
        .each_ref()
        .map(|s| canonical_field(s))
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    if fields[169 + i] != blinded_commitment(book, i, refund[0], refund[1], false)?
        || fields[173 + i] != blinded_commitment(book, i, winner[0], winner[1], true)?
    {
        return Err("ticket receipt mask/blinder/context/kind mismatch".into());
    }
    let bit = fields[183 + i] - winner[0];
    if bit != Fr::from(0) && bit != Fr::from(1) {
        return Err("ticket status is not a bit".into());
    }
    let won = bit == Fr::from(1);
    let amount: u64 = (fields[179 + i] - refund[0])
        .to_string()
        .parse()
        .map_err(|_| "ticket refund range")?;
    let price: u64 = fields[177]
        .to_string()
        .parse()
        .map_err(|_| "ticket price range")?;
    let sold: u32 = fields[178]
        .to_string()
        .parse()
        .map_err(|_| "ticket sold range")?;
    if price > book.budget
        || price < book.reserve
        || sold > book.units
        || sold > 4
        || amount != book.budget - if won { price } else { 0 }
    {
        return Err("ticket winning status/refund/price mismatch".into());
    }
    Ok(
        serde_json::json!({"ticket":receipt.ticket,"won":won,"refund":amount,"price":price,"sold":sold,
        "opening_verified":true,"proof_verified_here":false,"other_ticket_pads_read":false,
        "maximum_bids_read":false,"funding_or_chain_inclusion_verified":false}),
    )
}
pub(super) fn export_ticket(
    book: &Book,
    refund_masks: &Path,
    winner_masks: &Path,
    statement: &Path,
    index: usize,
    out: &Path,
) -> Result<()> {
    let receipt = make_ticket(book, refund_masks, winner_masks, statement, index)?;
    write_private(out, &serde_json::to_vec(&receipt)?)
}
fn make_ticket(
    book: &Book,
    refund_masks: &Path,
    winner_masks: &Path,
    statement: &Path,
    index: usize,
) -> Result<TicketReceipt> {
    if index >= 4 {
        return Err("ticket receipt index outside book".into());
    }
    let fields = ticket_statement(book, statement)?;
    // Only this operator-side exporter opens the complete set of delivery pads.
    // Recipients receive one pair for refund and one independent pair for status.
    let refund = masks(refund_masks)?;
    let winner = masks(winner_masks)?;
    let receipt = TicketReceipt {
        format: 1,
        rule: TICKET_RULE.into(),
        ticket: book.ids[index],
        accepted_root: book.root.clone(),
        refund_pad: refund[index].map(|f| f.to_string()),
        winner_pad: winner[index].map(|f| f.to_string()),
    };
    ticket_opening(book, &receipt, &fields)?;
    Ok(receipt)
}
pub(super) fn export_memo(
    book: &Book,
    refund_masks: &Path,
    winner_masks: &Path,
    statement: &Path,
    index: usize,
    out: &Path,
) -> Result<()> {
    let receipt = make_ticket(book, refund_masks, winner_masks, statement, index)?;
    write_private(out, &receipt.memo()?)
}
pub(super) fn open_memo(
    book: &Book,
    expected_index: usize,
    memo: &Path,
    statement: &Path,
) -> Result<serde_json::Value> {
    let receipt = TicketReceipt::from_memo(&bounded(memo, 512)?)?;
    if expected_index >= 4 || receipt.ticket != book.ids[expected_index] {
        return Err("memo differs from recipient's expected ticket".into());
    }
    ticket_opening(book, &receipt, &ticket_statement(book, statement)?)
}
pub(super) fn open_ticket(
    book: &Book,
    expected_index: usize,
    receipt: &Path,
    statement: &Path,
) -> Result<serde_json::Value> {
    let receipt: TicketReceipt = serde_json::from_slice(&bounded(receipt, 4096)?)?;
    if expected_index >= 4 || receipt.ticket != book.ids[expected_index] {
        return Err("receipt differs from recipient's expected ticket".into());
    }
    let fields = ticket_statement(book, statement)?;
    ticket_opening(book, &receipt, &fields)
}

pub(super) fn result(
    book: &Book,
    refund_masks: &Path,
    winner_masks: Option<&Path>,
    statement: &Path,
    out: &Path,
) -> Result<()> {
    book.validate_four_active()?;
    let flags = winner_masks.is_some();
    let count = if flags { 187 } else { 179 };
    if fs::metadata(statement)?.len() != count * 32 {
        return Err("expected versioned private delivery public statement".into());
    }
    let refund = masks(refund_masks)?;
    let bytes = bounded(statement, count * 32)?;
    if bytes.len() != count as usize * 32 {
        return Err("delivery statement size changed".into());
    }
    let fields = bytes
        .chunks_exact(32)
        .map(|b| {
            let f = Fr::from_be_bytes_mod_order(b);
            if encoded_field(f) != b {
                return Err("noncanonical private delivery field".into());
            }
            Ok(f)
        })
        .collect::<Result<Vec<_>>>()?;
    let legacy: Vec<Fr> = if flags {
        fields[..173]
            .iter()
            .chain(&fields[177..183])
            .copied()
            .collect()
    } else {
        fields.clone()
    };
    let pads: Vec<_> = refund.iter().map(|p| p[0]).collect();
    let expected = (0..4)
        .map(|i| blinded_commitment(book, i, refund[i][0], refund[i][1], false))
        .collect::<Result<Vec<_>>>()?;
    let mut opened = refund_statement_committed(book, &pads, &legacy, &expected)?;
    if let Some(path) = winner_masks {
        let winner = masks(path)?;
        let price = opened["price"].as_u64().ok_or("delivered price")?;
        let mut bits = [false; 4];
        for i in 0..4 {
            if fields[173 + i] != winner_commitment(book, i, winner[i][0], winner[i][1])? {
                return Err("winner mask/blind/context/kind/ticket mismatch".into());
            }
            let bit = fields[183 + i] - winner[i][0];
            if bit != Fr::from(0) && bit != Fr::from(1) {
                return Err("delivered winner is not a bit".into());
            }
            bits[i] = bit == Fr::from(1);
            if opened["amounts"][i].as_u64() != Some(book.budget - if bits[i] { price } else { 0 })
            {
                return Err("winner bit/refund mismatch".into());
            }
        }
        if bits.iter().filter(|v| **v).count() as u64
            != opened["sold"].as_u64().ok_or("delivered sold")?
        {
            return Err("winner count/sold mismatch".into());
        }
        opened["winners"] = serde_json::to_value(bits)?;
    }
    opened["delivery_rule"] = if flags {
        "fresh-field-mask-refund-and-winner-delivery-v2"
    } else {
        "fresh-field-mask-refund-delivery-v2"
    }
    .into();
    write_private(out, &serde_json::to_vec_pretty(&opened)?)
}

// Public-input-only dictionary regression. Never reads masks or bids/shares.
// v1 exposes two candidates exactly; v2 tests candidate masks with small blind
// guesses, not an exhaustive security proof against arbitrary blinding search.
pub(super) fn audit(
    book: &Book,
    statement: &Path,
    out: &Path,
    flags: bool,
    blinded: bool,
) -> Result<()> {
    let count = if flags { 187 } else { 179 };
    if fs::metadata(statement)?.len() != count * 32 {
        return Err("public audit statement size".into());
    }
    let bytes = bounded(statement, count * 32)?;
    if bytes.len() != count as usize * 32 {
        return Err("audit statement size changed".into());
    }
    let fields = bytes
        .chunks_exact(32)
        .map(|b| {
            let f = Fr::from_be_bytes_mod_order(b);
            if encoded_field(f) != b {
                return Err("noncanonical audit field".into());
            }
            Ok(f)
        })
        .collect::<Result<Vec<_>>>()?;
    if fields[..169] != book_fields(book)? {
        return Err("public audit context".into());
    }
    let price = fields[if flags { 177 } else { 173 }];
    let mut refund_recovered = 0;
    let mut winner_recovered = 0;
    for i in 0..4 {
        let public = fields[if flags { 179 + i } else { 175 + i }];
        for value in [Fr::from(book.budget), Fr::from(book.budget) - price] {
            let mask = public - value;
            let found = if blinded {
                (0..16).any(|b| {
                    blinded_commitment(book, i, mask, Fr::from(b), false)
                        .is_ok_and(|v| v == fields[169 + i])
                })
            } else {
                mask_commitment(book, i, mask)? == fields[169 + i]
            };
            if found {
                refund_recovered += 1;
                break;
            }
        }
        if flags {
            for bit in [Fr::from(0), Fr::from(1)] {
                let mask = fields[183 + i] - bit;
                let found = if blinded {
                    (0..16).any(|b| {
                        blinded_commitment(book, i, mask, Fr::from(b), true)
                            .is_ok_and(|v| v == fields[173 + i])
                    })
                } else {
                    let mut a = vec![
                        domain(b"DarkCall/winner-mask/v1"),
                        book.context(),
                        canonical_field(&book.root)?,
                    ];
                    a.extend(limbs(&book.ids[i]));
                    a.push(mask);
                    hash(&a) == fields[173 + i]
                };
                if found {
                    winner_recovered += 1;
                    break;
                }
            }
        }
    }
    write_private(
        out,
        &serde_json::to_vec(
            &serde_json::json!({"scope":"public-only candidate/mask dictionary regression; not exhaustive hiding proof",
        "blinded_commitment":blinded,"refunded_tickets_recovered":refund_recovered,"winner_bits_recovered":winner_recovered,
        "private_masks_bids_shares_read":false,"blind_guesses_per_candidate":if blinded {16} else {0}}),
        )?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_price_flags_survive_equal_refunds_and_reject_bad_openings() {
        let mut book = Book {
            condition_digest: [1; 32],
            ids: std::array::from_fn(|i| [i as u8; 32]),
            active: [true; 4],
            units: 3,
            reserve: 0,
            budget: 100,
            processing_fee: 15000,
            commitments: ["1", "2", "3", "4"].map(str::to_string),
            pair_commitments: std::array::from_fn(|_| std::array::from_fn(|_| "0".into())),
            root: String::new(),
        };
        book.root = book.root().unwrap().to_string();
        let dir = std::env::temp_dir().join(format!(
            "darkcall-flags-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let refund = [Fr::from(100), Fr::from(200), Fr::from(300), Fr::from(400)];
        let winner = [Fr::from(500), Fr::from(600), Fr::from(700), Fr::from(800)];
        let mut fields = book_fields(&book).unwrap();
        for (i, mask) in refund.iter().enumerate() {
            fields.push(
                blinded_commitment(&book, i, *mask, Fr::from(1000 + i as u64), false).unwrap(),
            );
        }
        for (i, mask) in winner.iter().enumerate() {
            fields.push(winner_commitment(&book, i, *mask, Fr::from(900 + i as u64)).unwrap());
        }
        fields.extend([Fr::from(0), Fr::from(3)]);
        fields.extend(refund.map(|v| v + Fr::from(100)));
        for (i, mask) in winner.iter().enumerate() {
            fields.push(*mask + Fr::from(u64::from(i < 3)));
        }
        let write = |values: &[Fr]| {
            fs::write(
                dir.join("statement"),
                values
                    .iter()
                    .copied()
                    .flat_map(encoded_field)
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        write(&fields);
        for (filename, values) in [("refund", refund), ("winner", winner)] {
            fs::write(
                dir.join(filename),
                serde_json::to_vec(&std::array::from_fn::<_, 4, _>(|i| {
                    [
                        values[i].to_string(),
                        (if filename == "refund" {
                            1000 + i
                        } else {
                            900 + i
                        })
                        .to_string(),
                    ]
                }))
                .unwrap(),
            )
            .unwrap();
        }
        let open = |name: &str| {
            result(
                &book,
                &dir.join("refund"),
                Some(dir.join("winner").as_path()),
                &dir.join("statement"),
                &dir.join(name),
            )
        };
        open("valid").unwrap();
        let opened: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("valid")).unwrap()).unwrap();
        assert_eq!(
            opened["winners"],
            serde_json::json!([true, true, true, false])
        );
        assert_eq!(
            opened["amounts"],
            serde_json::json!([100, 100, 100, 100, 0, 30000])
        );
        // The per-ticket path does not read all operator pads. The claimant's
        // expected ticket must come from their own registration, not the receipt.
        for i in 0..4 {
            let receipt = dir.join(format!("ticket-{i}"));
            export_ticket(
                &book,
                &dir.join("refund"),
                &dir.join("winner"),
                &dir.join("statement"),
                i,
                &receipt,
            )
            .unwrap();
            let one = open_ticket(&book, i, &receipt, &dir.join("statement")).unwrap();
            assert_eq!(one["won"], i < 3);
            assert_eq!(one["refund"], 100);
            assert_eq!(one["proof_verified_here"], false);
            assert!(open_ticket(&book, (i + 1) % 4, &receipt, &dir.join("statement")).is_err());
            let memo = dir.join(format!("memo-{i}"));
            export_memo(
                &book,
                &dir.join("refund"),
                &dir.join("winner"),
                &dir.join("statement"),
                i,
                &memo,
            )
            .unwrap();
            assert_eq!(
                open_memo(&book, i, &memo, &dir.join("statement")).unwrap(),
                one
            );
            assert!(open_memo(&book, (i + 1) % 4, &memo, &dir.join("statement")).is_err());
            let encoded = fs::read(memo).unwrap();
            assert_eq!(
                TicketReceipt::from_memo(&encoded)
                    .unwrap()
                    .memo()
                    .unwrap()
                    .as_slice(),
                encoded
            );
            for position in [0, 1, 8, MEMO_USED, 511] {
                let mut changed = encoded.clone();
                changed[position] ^= 1;
                assert!(TicketReceipt::from_memo(&changed).is_err());
            }
            let mut noncanonical = encoded.clone();
            noncanonical[41..73].fill(0xff);
            assert!(TicketReceipt::from_memo(&noncanonical).is_err());
            assert!(TicketReceipt::from_memo(&encoded[..511]).is_err());
            assert!(TicketReceipt::from_memo(&[encoded.as_slice(), &[0]].concat()).is_err());
            let mut wire: serde_json::Value =
                serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
            wire["won"] = true.into();
            assert!(serde_json::from_value::<TicketReceipt>(wire).is_err());
        }
        for (name, index, delta) in [
            ("not-a-bit", 183, 1),
            ("too-many-winners", 186, 1),
            ("too-few-winners", 183, -1),
        ] {
            let mut changed = fields.clone();
            changed[index] += if delta > 0 { Fr::from(1) } else { -Fr::from(1) };
            write(&changed);
            assert!(open(name).is_err());
            assert!(!dir.join(name).exists());
        }
        write(&fields);
        fs::write(
            dir.join("winner"),
            serde_json::to_vec(&std::array::from_fn::<_, 4, _>(|i| {
                [refund[i].to_string(), (1000 + i).to_string()]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(open("wrong-mask-kind").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
