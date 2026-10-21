// Spike: the bidder client inside a browser page. Same construction as client.mjs;
// the bid is read from the page and only the commitment, proof, public inputs and
// one share pair per operator are posted. CRS points come from this origin.
import { Barretenberg, BackendType, UltraHonkBackend } from '@aztec/bb.js';
import { Noir } from '@noir-lang/noir_js';
import initACVM from '@noir-lang/acvm_js';
import initAbi from '@noir-lang/noirc_abi';
import { set } from 'idb-keyval';

const P = 21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const mod = x => ((x % P) + P) % P;
const toBytes = x => { const b = new Uint8Array(32); let v = mod(x); for (let i = 31; i >= 0; i--) { b[i] = Number(v & 255n); v >>= 8n; } return b; };
const fromBytes = b => b.reduce((v, x) => (v << 8n) | BigInt(x), 0n);
const randomField = () => mod(fromBytes(crypto.getRandomValues(new Uint8Array(64))));
const domain = text => fromBytes(new TextEncoder().encode(text));
const limbs = bytes => [0, 1, 2, 3].map(i => fromBytes(Uint8Array.from(bytes.slice(8 * i, 8 * i + 8))));
const b64 = bytes => btoa(Array.from(bytes, c => String.fromCharCode(c)).join(''));
const bytesOf = async url => new Uint8Array(await (await fetch(url)).arrayBuffer());
const log = text => { document.querySelector('#log').textContent += text + '\n'; };

async function seal(bid) {
  const timings = {};
  let t = performance.now();
  const auction = await (await fetch('auction.json')).json();
  await Promise.all([initACVM(fetch('acvm_js_bg.wasm')), initAbi(fetch('noirc_abi_wasm_bg.wasm'))]);
  const circuit = await (await fetch('darkcall_bid_input.json')).json();
  // Public CRS points, served by this origin and cached where bb.js looks first.
  await set('g1Data', await bytesOf('crs/g1'));
  await set('g2Data', await bytesOf('crs/g2'));
  await set('grumpkinG1Data', await bytesOf('crs/grumpkin'));
  const threads = self.crossOriginIsolated ? Math.min(navigator.hardwareConcurrency || 4, 8) : 1;
  const api = await Barretenberg.new({ backend: BackendType.WasmWorker, threads, srsSize: auction.srs_points });
  timings.load_ms = Math.round(performance.now() - t);

  const permute = async state => (await api.poseidon2Permutation({ inputs: state.map(toBytes) })).outputs.map(fromBytes);
  const hash = async input => {
    let state = [0n, 0n, 0n, mod(BigInt(input.length) << 64n)];
    const full = input.length - (input.length % 3);
    for (let c = 0; c < full; c += 3) {
      for (let i = 0; i < 3; i++) state[i] = mod(state[i] + input[c + i]);
      state = await permute(state);
    }
    input.slice(full).forEach((v, i) => { state[i] = mod(state[i] + v); });
    return (await permute(state))[0];
  };
  const share = x => {
    const x0 = randomField(), x1 = randomField(), xs = [x0, x1, mod(x - x0 - x1)];
    return { additive: xs, pairs: [0, 1, 2].map(p => ({ a: xs[p], b: xs[(p + 2) % 3] })) };
  };
  const shareFile = (i, b, s) => {
    const u64 = n => { const x = new Uint8Array(8); new DataView(x.buffer).setBigUint64(0, BigInt(n), true); return x; };
    const parts = [u64(2)];
    for (const [key, v] of [['bid' + i, b], ['salt' + i, s]]) {
      const k = new TextEncoder().encode(key);
      parts.push(u64(k.length), k, Uint8Array.of(1, 0, 0, 0), u64(64), toBytes(v.a).reverse(), toBytes(v.b).reverse());
    }
    const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
    parts.reduce((at, p) => (out.set(p, at), at + p.length), 0);
    return out;
  };

  t = performance.now();
  const { condition_digest: cond, id, bidder: i, units, reserve, budget, processing_fee: fee } = auction;
  const ctx = await hash([domain('DarkCall/ctx/v1'), ...limbs(cond), BigInt(units), BigInt(reserve), BigInt(budget), BigInt(fee)]);
  const salt = randomField(), bids = share(bid), salts = share(salt);
  const commitment = await hash([domain('DarkCall/bid/v1'), ctx, ...limbs(id), bid, salt]);
  const pairs = [];
  for (let p = 0; p < 3; p++) {
    const [b, s] = [bids.pairs[p], salts.pairs[p]];
    pairs.push(await hash([domain('DarkCall/share/v1'), ctx, ...limbs(id), BigInt(p), b.a, b.b, s.a, s.b]));
  }
  const active = [0, 1, 2, 3].map(n => n === i), commitments = active.map(a => (a ? commitment : 0n));
  const root = await hash([domain('DarkCall/accepted/v1'), ctx, 4n, 1n, ...active.map(x => BigInt(x)), ...commitments]);
  timings.commit_and_share_ms = Math.round(performance.now() - t);

  t = performance.now();
  const { witness } = await new Noir(circuit).execute({
    bid: bid.toString(), salt: salt.toString(), bid_shares: bids.additive.map(String), salt_shares: salts.additive.map(String),
    condition_digest: cond, id, units, reserve, budget, processing_fee: fee,
    commitment: commitment.toString(), pair_commitments: pairs.map(String),
  });
  timings.witness_ms = Math.round(performance.now() - t);
  t = performance.now();
  const proof = await new UltraHonkBackend(circuit.bytecode, api).generateProof(witness, { verifierTarget: 'noir-recursive' });
  timings.proof_ms = Math.round(performance.now() - t);
  await api.destroy();

  // Everything that leaves the page. No bid, salt or full share set.
  const body = {
    book: { condition_digest: cond, ids: active.map(a => (a ? id : Array(32).fill(0))), active, units, reserve, budget,
      processing_fee: fee, commitments: commitments.map(String),
      pair_commitments: active.map(a => (a ? pairs.map(String) : ['0', '0', '0'])), root: String(root) },
    proof: b64(proof.proof),
    public_inputs: b64(new Uint8Array(proof.publicInputs.flatMap(h => Array.from(toBytes(BigInt(h)))))),
    // One pair per operator. In a real deployment each is encrypted to that operator.
    share_pairs: [0, 1, 2].map(p => b64(shareFile(i, bids.pairs[p], salts.pairs[p]))),
  };
  const response = await fetch('submit', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  return { threads, crossOriginIsolated: self.crossOriginIsolated, timings, submitted: response.status,
    posted_keys: Object.keys(body), book_keys: Object.keys(body.book) };
}

document.querySelector('#seal').addEventListener('click', async () => {
  const bid = BigInt(document.querySelector('#bid').value);
  document.querySelector('#bid').value = '';
  try {
    window.__result = await seal(bid);
    log(JSON.stringify(window.__result, null, 1));
  } catch (error) {
    window.__result = { error: String(error && error.stack || error) };
    log(window.__result.error);
  }
});
