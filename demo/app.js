'use strict';
const $ = id => document.getElementById(id);
let state, selected = 'positive', busy = false, receiptVerified = false, settlementVerified = false;
function stages() {
  $('stage-status').textContent = (receiptVerified ? 'Clearing and your result verified. ' : 'Recorded clearing; own result unopened. ') + (settlementVerified ? 'Shielded settlement verified.' : 'Shielded settlement awaits public verification.');
  const checks = $('result-checks'); checks.replaceChildren();
  if (receiptVerified) checks.append(element('span','✓ Auction & receipt verified'),element('span','✓ Native Zcash transaction'));
  if (settlementVerified && state.six_output_memo_e2e) checks.append(element('span','✓ Settlement verified'));
  checks.hidden = !checks.childElementCount;
}
const setError = message => { $('error').textContent = message; $('error').hidden = !message; };
function element(tag, text, className) { const node = document.createElement(tag); node.textContent = text; if (className) node.className = className; return node; }
function resetResult() {
  receiptVerified = false; stages();
  const result = $('result'); result.className = 'result-box'; result.replaceChildren(
    element('h3','Still sealed.'), element('p','Open your encrypted receipt to see your result.'));
  ['proof','native','open'].forEach(step => $('step-'+step).classList.remove('done'));
  $('verify').replaceChildren(document.createTextNode('Open my result '), element('span','↗'));
  $('tamper-result').textContent = ''; setError('');
}
function renderCase() {
  const current = state.cases.find(c => c.id === selected);
  $('price').textContent = current.price;
  $('sold').replaceChildren(document.createTextNode(current.sold),element('i','/ 3'));
  $('root').textContent = current.root;
  $('root').title = current.root;
  $('txid').textContent = current.txid;
  $('fee').textContent = current.fee.toLocaleString() + ' zatoshi';
  document.querySelectorAll('[data-case]').forEach(button => {
    button.classList.toggle('active', button.dataset.case === selected);
    button.setAttribute('aria-pressed', String(button.dataset.case === selected));
  }); resetResult();
}
function lock(value) { busy = value; document.querySelectorAll('button,select').forEach(node => node.disabled = value); if (!value && state && !state.recipient_identities_available) { $('verify').disabled=true; $('ticket').disabled=true; } }
async function run(action) {
  if (busy || !state) return;
  const ticket = Number($('ticket').value);
  lock(true); setError('');
  if (action === 'verify') {
    resetResult(); $('result').className = 'result-box loading';
    $('result').replaceChildren(element('h3','Verifying…'),element('p','Checking the proof and opening only your receipt.'));
  } else $('tamper-result').textContent = 'Submitting a corrupted proof to the real verifier…';
  try {
    const response = await fetch('/api/verify', {method:'POST',headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:JSON.stringify({case:selected,ticket,action})});
    const result = await response.json(); if (!response.ok) throw new Error(result.error || 'Verification failed');
    if (action === 'tamper') {
      if (!result.rejected || result.key_read || result.memo_decrypted) throw new Error('Unexpected tamper result');
      $('tamper-result').textContent = '✓ Cheating rejected. Viewing key untouched. Receipt stays encrypted.';
    } else {
      if (!result.fixed_cpp_ticket_proof_verified || !result.native_proof_and_signatures_verified || !result.own_ticket_opening_verified || !result.native_amount_matches_refund || result.ticket_index !== ticket) throw new Error('Incomplete verification result');
      const current = state.cases.find(c => c.id === selected);
      if (result.txid !== current.txid || result.price !== current.price || result.sold !== current.sold || (state.six_output_memo_e2e && (!result.receipt_in_six_output_transaction || !result.fixed_auction_context_proof_verified))) throw new Error('Receipt belongs to a different auction');
      receiptVerified = true; stages();
      const box = $('result'); box.className = 'result-box verified';
      box.replaceChildren(element('h3',result.won ? 'YOU WON' : 'YOU LOST'),element('p',result.won ? '1 unit @ '+result.price+' zatoshi' : 'Your refund is verified.'));
      const amounts = element('div','','receipt-amounts');
      for (const [label,amount] of [['Refund · zatoshi',result.refund]]) {
        const column = element('div',''); column.append(element('span',label),element('strong',String(amount))); amounts.append(column);
      }
      box.append(amounts);
      ['proof','native','open'].forEach(step => $('step-'+step).classList.add('done'));
      $('verify').replaceChildren(document.createTextNode('Verify again '),element('span','↗'));
    }
  } catch (error) { if (action === 'verify') resetResult(); else $('tamper-result').textContent=''; setError(error.message); }
  finally { $('result').classList.remove('loading'); lock(false); }
}
document.querySelectorAll('[data-case]').forEach(button => button.addEventListener('click',() => { if (!busy && state) { selected=button.dataset.case; renderCase(); } }));
$('ticket').addEventListener('change',resetResult);
$('verify').addEventListener('click',() => run('verify'));
$('tamper').addEventListener('click',() => run('tamper'));
$('settlement').addEventListener('click', async () => {
  if (busy || !state) return;
  lock(true); setError('');
  $('settlement-result').textContent = 'Checking the shielded transaction against the auction. Public verification usually takes about two minutes on this Mac…';
  try {
    const response = await fetch('/api/settlement', {method:'POST', headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:'{}'});
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || 'Settlement verification failed');
    if (!result.native_row_proofs_verified || !result.native_proof_and_signatures_verified || !result.normal_cpp_auction_proof_verified || !result.actual_transaction_outputs_match || result.private_openings_read || result.chain_funding_or_finality_verified) throw new Error('Incomplete settlement verification');
    if (state.six_output_memo_e2e && (result.txid !== state.cases[0].txid || !result.receipt_fixed_proof_verified || !result.receipt_settlement_context_link_verified)) throw new Error('Settlement belongs to a different receipt auction');
    settlementVerified = true; stages();
    $('settlement-result').textContent = '✓ Settlement verified. All six outputs match the auction. No private openings read.';
    $('settlement-detailed-result').textContent = 'Native authorization, six bindings, fixed auction/receipt proofs and context verified. ' + (result.acceptance_or_recipient_receipts_authenticated ? 'Accepted-book signatures verified. ' : 'No funded admission certificate for this synthetic run. ') + 'Transaction: '+result.txid;
  } catch (error) { $('settlement-result').textContent=''; setError(error.message); }
  finally { lock(false); }
});
lock(true);
fetch('/api/state').then(async response => {
  const data=await response.json(); if (!response.ok) throw new Error(data.error || 'Demo unavailable'); state=data;
  if (state.six_output_memo_e2e) {
    $('case-tabs').hidden = true;
    $('scope-description').textContent = 'Recorded proof-backed demo · synthetic funding, no broadcast.';
    $('native-shape').textContent = 'Ironwood · NU7 · 6 outputs / 4 encrypted receipts';
    $('settlement-scope').textContent = 'Verify that the shielded Zcash transaction matches the auction outcome — without seeing the bids.';
    $('remaining-scope').textContent = 'Local trusted-dealer threshold setup; all parties on one host. Public testnet funding, DKG admission and confirmations remain to be integrated into this same NU7 execution.';
    if (state.funded_canonical) {
      $('scope-description').textContent = 'Recorded proof-backed demo on isolated Zcash regtest.';
      $('remaining-scope').textContent = 'Recorded fixture auction; no live browser bid admission. Independent-host operation, public testnet, broader wallet recovery and larger bid counts remain unverified.';
      $('chain-scope').textContent = state.recorded_local_confirmations ? 'Two controlled regtest confirmations recorded; public bundle does not prove chain inclusion' : 'Recorded regtest run; no chain inclusion proof in the public bundle';
    }
    if (!state.recipient_identities_available) {
      $('result').replaceChildren(element('h3','Public verification view.'),element('p','This checkout contains no recipient keys. You can verify settlement and reject tampering; own receipt opening requires your private identity.'));
    }
  } else {
    $('scope-description').textContent = 'Historical component demo · receipt and settlement are separate transactions.';
    $('native-shape').textContent = 'Ironwood · NU7 · 4 receipt outputs';
    $('settlement-scope').textContent = 'Historical price-25 isolated-regtest settlement without receipt memos. This is separate from the selected receipt component.';
    $('remaining-scope').textContent = 'Use ./demo.sh for the coherent six-output auction view.';
  }
  renderCase();
  if (!state.recipient_identities_available) $('result').replaceChildren(element('h3','Public verification view.'),element('p','No recipient keys are included in Git. Verify settlement or run the tamper check.'));
  if (state.last_settlement) {
    $('settlement-result').textContent = 'Previous public check passed. Run again to reverify.';
    $('settlement-detailed-result').textContent = 'Previous public verification passed for transaction: '+state.last_settlement.txid+'. Not reverified in this page session.';
  }
  lock(false);
}).catch(error => setError(error.message));
