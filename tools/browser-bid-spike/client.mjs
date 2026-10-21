// Spike: everything the bidder client does, with no Rust tool and no server.
// Commitments (Poseidon2 sponge as in crates/input-tool/src/lib.rs), REP3 share
// pairs (mpc-core layout), witness (noir_js), proof (bb.js WASM, noir-recursive),
// and the operator files in the exact formats prepare-one + plaindriver write.
// Usage: node client.mjs <node_modules> <repo> <out-dir> <bidder 0-3> <bid> [threads] [--check-ref <ref-dir>]
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { webcrypto } from 'crypto';

const [modules, repo, out, bidderText, bidText, threadsText = '1', ...rest] = process.argv.slice(2);
const bb = await import(modules + '/@aztec/bb.js/dest/node/index.js');
const { Noir } = await import(modules + '/@noir-lang/noir_js/lib/index.mjs');
const P = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const mod = x => ((x % P) + P) % P;
const toBytes = x => { const b = new Uint8Array(32); let v = mod(x); for (let i = 31; i >= 0; i--) { b[i] = Number(v & 255n); v >>= 8n; } return b; };
const fromBytes = b => b.reduce((v, x) => (v << 8n) | BigInt(x), 0n);
const randomField = () => mod(fromBytes(webcrypto.getRandomValues(new Uint8Array(64))));
const domain = text => fromBytes(new TextEncoder().encode(text));
const limbs = bytes => [0, 1, 2, 3].map(i => fromBytes(bytes.slice(8 * i, 8 * i + 8)));

const timings = {};
let t = performance.now();
const api = await bb.Barretenberg.new({ backend: bb.BackendType.WasmWorker, threads: Number(threadsText),
  crsPath: out + '/../crs', logger: m => /Download/i.test(m) && console.error('CRS:', m) });
timings.backend_init_ms = Math.round(performance.now() - t);

async function permute(state) {
  const { outputs } = await api.poseidon2Permutation({ inputs: state.map(toBytes) });
  return outputs.map(fromBytes);
}
// crates/input-tool/src/lib.rs::hash, line for line.
async function hash(input) {
  let state = [0n, 0n, 0n, mod(BigInt(input.length) << 64n)];
  const full = input.length - (input.length % 3);
  for (let c = 0; c < full; c += 3) {
    for (let i = 0; i < 3; i++) state[i] = mod(state[i] + input[c + i]);
    state = await permute(state);
  }
  input.slice(full).forEach((v, i) => { state[i] = mod(state[i] + v); });
  return (await permute(state))[0];
}
const context = (cond, units, reserve, budget, fee) =>
  hash([domain('DarkCall/ctx/v1'), ...limbs(cond), BigInt(units), BigInt(reserve), BigInt(budget), BigInt(fee)]);
const leaf = (ctx, id, bid, salt) => hash([domain('DarkCall/bid/v1'), ctx, ...limbs(id), BigInt(bid), salt]);
const pair = (ctx, id, party, b, s) => hash([domain('DarkCall/share/v1'), ctx, ...limbs(id), BigInt(party), b.a, b.b, s.a, s.b]);
const root = async (ctx, active, commitments) => hash([domain('DarkCall/accepted/v1'), ctx, 4n,
  BigInt(active.filter(Boolean).length), ...active.map(x => BigInt(x)), ...commitments]);
// mpc-core rep3::share_field_element: x = x0 + x1 + x2; party p holds (x_p, x_{p-1}).
function share(x) {
  const x0 = randomField(), x1 = randomField(), x2 = mod(x - x0 - x1);
  const xs = [x0, x1, x2];
  return { additive: xs, pairs: [0, 1, 2].map(p => ({ a: xs[p], b: xs[(p + 2) % 3] })) };
}
// bincode (fixint) of BTreeMap<String, Rep3Type::Shared(Rep3PrimeFieldShare)>, as prepare-one writes it.
function shareFile(bidder, b, s) {
  const parts = [];
  const u64 = n => { const x = new Uint8Array(8); new DataView(x.buffer).setBigUint64(0, BigInt(n), true); return x; };
  const le = v => toBytes(v).reverse();
  parts.push(u64(2));
  for (const [key, v] of [['bid' + bidder, b], ['salt' + bidder, s]]) {
    const k = new TextEncoder().encode(key);
    parts.push(u64(k.length), k, Uint8Array.of(1, 0, 0, 0), u64(64), le(v.a), le(v.b));
  }
  return Buffer.concat(parts);
}

if (rest[0] === '--check-ref') {
  // Recompute prepare-one's commitments from its own Prover.toml: same hash, same layout?
  const dir = rest[1];
  const book = JSON.parse(readFileSync(dir + '/book.json'));
  const toml = Object.fromEntries(readFileSync(dir + '/Prover.toml', 'utf8').trim().split('\n').map(l => {
    const [k, v] = l.split(' = '); return [k, JSON.parse(v)]; }));
  const i = book.active.indexOf(true);
  const ctx = await context(book.condition_digest, book.units, book.reserve, book.budget, book.processing_fee);
  const xs = toml.bid_shares.map(BigInt), ss = toml.salt_shares.map(BigInt);
  const checks = {
    commitment: (await leaf(ctx, book.ids[i], toml.bid, BigInt(toml.salt))).toString() === book.commitments[i],
    pairs: await Promise.all([0, 1, 2].map(async p => (await pair(ctx, book.ids[i], p, { a: xs[p], b: xs[(p + 2) % 3] },
      { a: ss[p], b: ss[(p + 2) % 3] })).toString() === book.pair_commitments[i][p])),
    root: (await root(ctx, book.active, book.commitments.map(BigInt))).toString() === book.root,
    share_files_byte_identical: [0, 1, 2].map(p => Buffer.compare(shareFile(i, { a: xs[p], b: xs[(p + 2) % 3] },
      { a: ss[p], b: ss[(p + 2) % 3] }), readFileSync(`${dir}/bidder${i}.toml.${p}.shared`)) === 0),
  };
  console.log(JSON.stringify({ reference: dir, checks }, null, 1));
  await api.destroy();
  process.exit(0);
}

// A fresh synthetic bid, built only here.
const bidder = Number(bidderText), bid = BigInt(bidText);
const terms = { units: 3, reserve: 20, budget: 100, processing_fee: 15000 };
// The auction's public condition digest (from the operators' init), or a random one.
const condition = process.env.CONDITION ? Array.from(Buffer.from(process.env.CONDITION, 'hex'))
  : Array.from(webcrypto.getRandomValues(new Uint8Array(32)));
const id = Array.from(webcrypto.getRandomValues(new Uint8Array(32)));
t = performance.now();
const ctx = await context(condition, terms.units, terms.reserve, terms.budget, terms.processing_fee);
const salt = randomField();
const bids = share(bid), salts = share(salt);
const commitment = await leaf(ctx, id, bid, salt);
const pairs = [];
for (let p = 0; p < 3; p++) pairs.push(await pair(ctx, id, p, bids.pairs[p], salts.pairs[p]));
const active = [0, 1, 2, 3].map(n => n === bidder);
const commitments = active.map(a => (a ? commitment : 0n));
const book = {
  condition_digest: condition, ids: [0, 1, 2, 3].map(n => (n === bidder ? id : Array(32).fill(0))), active, ...terms,
  commitments: commitments.map(String),
  pair_commitments: [0, 1, 2, 3].map(n => (n === bidder ? pairs.map(String) : ['0', '0', '0'])),
  root: String(await root(ctx, active, commitments)),
};
timings.commitments_and_shares_ms = Math.round(performance.now() - t);

const circuit = JSON.parse(readFileSync(repo + '/circuits/bid_input/target/darkcall_bid_input.json'));
t = performance.now();
const { witness } = await new Noir(circuit).execute({
  bid: bid.toString(), salt: salt.toString(), bid_shares: bids.additive.map(String), salt_shares: salts.additive.map(String),
  condition_digest: condition, id, units: terms.units, reserve: terms.reserve, budget: terms.budget,
  processing_fee: terms.processing_fee, commitment: commitment.toString(), pair_commitments: pairs.map(String),
});
timings.witness_ms = Math.round(performance.now() - t);
t = performance.now();
const backend = new bb.UltraHonkBackend(circuit.bytecode, api);
const proof = await backend.generateProof(witness, { verifierTarget: 'noir-recursive' });
timings.proof_ms = Math.round(performance.now() - t);
t = performance.now();
const selfVerified = await backend.verifyProof(proof, { verifierTarget: 'noir-recursive' });
timings.bbjs_verify_ms = Math.round(performance.now() - t);

// What leaves the client: book.json (public), proof + public inputs, and one share file per operator.
mkdirSync(out, { recursive: true });
writeFileSync(out + '/book.json', JSON.stringify(book));
writeFileSync(out + '/proof_plaindriver', proof.proof);
writeFileSync(out + '/public_inputs_plaindriver', Buffer.concat(proof.publicInputs.map(h => toBytes(BigInt(h)))));
for (let p = 0; p < 3; p++) writeFileSync(`${out}/bidder${bidder}.toml.${p}.shared`, shareFile(bidder, bids.pairs[p], salts.pairs[p]));
console.log(JSON.stringify({ bidder, public_inputs: proof.publicInputs.length, proof_bytes: proof.proof.length,
  bbjs_self_verify: selfVerified, threads: Number(threadsText), timings }, null, 1));
await api.destroy();
