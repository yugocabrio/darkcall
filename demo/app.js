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
      $('remaining-scope').textContent = 'This recorded auction is separate from fresh local clearing. New books use synthetic deposits and a helper that sees typed demo bids. Their outcomes are proof-verified, but native settlement is not constructed. Independent hosts, public testnet and larger bid counts remain unverified.';
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
  $('recording-banner').textContent=(state.funded_canonical ? 'RECORDED FUNDED AUCTION' : 'RECORDED DEMO')+' · These results belong to the recorded book, not the new rehearsal bids.';
  refreshRoom(); joinRehearsal();
}).catch(error => setError(error.message));

// Interactive entry is an OFFLINE rehearsal. It never alters the recorded book.
let room = null, lastAcceptance = null;
const integer = id => { const text=$(id).value; if (!/^\d+$/.test(text)) throw new Error('Use whole-number demo values.'); return Number(text); };
function rehearsalError(message) { $('rehearsal-error').textContent=message; $('rehearsal-error').hidden=!message; }
function refreshRoom() {
  const available=Boolean(state && state.local_rehearsal_available);
  $('create-auction').disabled=busy || !available;
  if (!room) return;
  const remaining=Math.max(0,room.closes_at_utc-Math.floor(Date.now()/1000));
  $('room-terms').textContent=room.units+' units · reserve '+room.reserve+' · budget '+room.budget+' zatoshi';
  $('room-count').textContent=room.accepted+' / 4 accepted';
  $('maximum-bid').max=room.budget;
  $('seal-bid').disabled=busy || !available || room.stopped_after_incomplete_admission || remaining===0 || room.accepted>=4 || room.closed;
  $('close-auction').disabled=busy || !available || room.stopped_after_incomplete_admission || remaining>0 || room.accepted!==4 || room.closed;
  $('clear-auction').disabled=busy || !available || !room.clearing_available || !room.closed || room.clearing_stopped || Boolean(room.clearing);
  $('open-fresh-result').disabled=busy || !room.clearing || !lastAcceptance;
  $('tamper-fresh').disabled=busy || !room.clearing || !lastAcceptance;
  $('close-status').textContent=room.stopped_after_incomplete_admission ? 'Admission stopped. Start a new rehearsal; the incomplete checkpoint is preserved.' : room.closed ? '✓ Book closed. Three final-book signatures verified.' :
    remaining>0 ? 'Bidding closes in '+Math.floor(remaining/60)+':'+String(remaining%60).padStart(2,'0')+'. '+room.deposit+' zatoshi per synthetic deposit.' :
    room.accepted===4 ? 'Bidding is closed. Certify the four accepted tickets.' : 'Bidding is closed; four tickets are required to certify this prototype book.';
}
function showRoom(value) {
  if (!value || value.funding_scope!=='offline-synthetic' || value.chain_funding_verified || value.native_settlement_started) throw new Error('Unexpected rehearsal scope.');
  if (room && room.auction_id!==value.auction_id) {
    lastAcceptance=null; $('bid-accepted').hidden=true; $('copy-acceptance').hidden=true; $('acceptance-details').hidden=true;
    resetFresh();
  }
  room=value; $('create-form').hidden=true; $('auction-room').hidden=false;
  $('rehearsal-title').textContent='Place a sealed demo bid.';
  $('auction-link').value=location.origin+'/#auction='+room.auction_id;
  refreshRoom();
  if (room.clearing) renderFresh(room.clearing);
}
async function rehearsalRequest(action,payload) {
  const response=await fetch('/api/rehearsal/'+action,{method:'POST',headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:JSON.stringify(payload)});
  const result=await response.json(); if (!response.ok) throw new Error(result.error || 'Local rehearsal stopped.'); return result;
}
function resetFresh() {
  $('fresh-outcome').hidden=true; $('clearing-status').textContent=''; $('fresh-tamper-result').textContent='';
  $('fresh-result').className='result-box';
  $('fresh-result').replaceChildren(element('h3','Still sealed.'),element('p','Open the ticket accepted in this browser session.'));
}
function renderFresh(result) {
  if (!room || result.auction_id!==room.auction_id || result.accepted_root!==room.accepted_root
      || !result.rep3_clearing_verified || !result.fixed_cpp_proof_verified || !result.certified_book_matches
      || result.certificate_signatures_verified!==3 || result.native_settlement) throw new Error('Clearing does not match this certified book.');
  $('fresh-outcome').hidden=false;
  $('fresh-price').textContent=result.price;
  $('fresh-sold').textContent=result.sold+' / '+room.units;
  $('clearing-status').textContent='✓ This auction cleared. Fresh proof verified in '+result.seconds.toFixed(1)+' seconds.';
  $('fresh-proof-details').textContent='REP3 delivery_flags4/v2 · fixed C++ verification · three final-book signatures. Proof generation: '+result.proof_generation_seconds.toFixed(1)+' seconds.';
  $('fresh-root').textContent='Same accepted root: '+result.accepted_root;
}
$('clear-auction').addEventListener('click',async()=>{
  if (busy || !room) return; rehearsalError(''); lock(true);
  $('clearing-status').textContent='Clearing these secret shares and generating a fresh MPC proof…';
  try {
    const result=await rehearsalRequest('clear',{auction_id:room.auction_id});
    renderFresh(result); room.clearing=result; room.fresh_clearing_verified=true;
  } catch(error) {
    $('clearing-status').textContent=''; rehearsalError(error.message);
    try { const response=await fetch('/api/rehearsal/auction/'+room.auction_id); if (response.ok) showRoom(await response.json()); } catch { /* Preserve failure. */ }
  } finally { lock(false); refreshRoom(); }
});
async function openFresh(action) {
  if (busy || !room || !lastAcceptance || !room.clearing) return;
  rehearsalError(''); lock(true);
  if (action==='open') $('fresh-result').replaceChildren(element('h3','Verifying…'),element('p','Checking the proof and this signed book before opening your result.'));
  else $('fresh-tamper-result').textContent='Submitting a corrupted clearing proof to the fixed verifier…';
  try {
    const result=await rehearsalRequest('result',{auction_id:room.auction_id,ticket:lastAcceptance.ticket,opening_token:lastAcceptance.opening_token,action});
    if (result.auction_id!==room.auction_id || result.ticket!==lastAcceptance.ticket || result.native_settlement) throw new Error('Unexpected result scope.');
    if (action==='tamper') {
      if (!result.rejected || result.private_pad_read) throw new Error('Unexpected tamper check.');
      $('fresh-tamper-result').textContent='✓ Corrupted proof rejected. Your ticket pads were not read.';
    } else {
      if (!result.fixed_cpp_proof_verified || !result.certified_book_matches || result.accepted_root!==room.accepted_root
          || result.price!==room.clearing.price || result.sold!==room.clearing.sold || !result.opening_verified) throw new Error('Incomplete own-result verification.');
      const box=$('fresh-result'); box.className='result-box verified';
      box.replaceChildren(element('h3',result.won?'YOU WON':'YOU LOST'),element('p',result.won?'1 unit @ '+result.price+' zatoshi':'No unit allocated. Your outcome is verified.'));
      const amounts=element('div','','receipt-amounts'), column=element('div','');
      column.append(element('span','Refund entitlement · zatoshi'),element('strong',String(result.refund))); amounts.append(column); box.append(amounts,element('p','✓ Auction verified · synthetic deposit, no payment sent'));
    }
  } catch(error) {
    if (action==='open') {
      $('fresh-result').className='result-box';
      $('fresh-result').replaceChildren(element('h3','Opening stopped.'),element('p','No verified result was opened.'));
    } else $('fresh-tamper-result').textContent='';
    rehearsalError(error.message);
  }
  finally { lock(false); refreshRoom(); }
}
$('open-fresh-result').addEventListener('click',()=>openFresh('open'));
$('tamper-fresh').addEventListener('click',()=>openFresh('tamper'));
$('start-auction').addEventListener('click',()=>{
  $('rehearsal').hidden=false; $('recorded-demo').hidden=true;
  if (!room) setDefaultClose();
  $('rehearsal-status').textContent=state && state.local_rehearsal_available ? '' : 'Local input prover and admission helper are required; recorded verification is available below.';
  refreshRoom(); $('rehearsal').scrollIntoView({behavior:'smooth',block:'start'});
});
function showRecording() {
  $('rehearsal').hidden=true; $('recorded-demo').hidden=false;
  // Never use the new maximum bid or receipt to choose an old fixture identity.
  $('recorded-demo').scrollIntoView({behavior:'smooth',block:'start'});
}
$('show-recording').addEventListener('click',showRecording);
$('continue-recording').addEventListener('click',showRecording);
$('new-auction').addEventListener('click',()=>{
  if (busy) return;
  room=null; lastAcceptance=null; history.replaceState(null,'',location.pathname);
  resetFresh();
  $('auction-room').hidden=true; $('create-form').hidden=false; $('rehearsal-title').textContent='Create your auction.';
  $('bid-accepted').hidden=true; $('copy-acceptance').hidden=true; $('acceptance-details').hidden=true;
  $('rehearsal-status').textContent=''; rehearsalError('');
  setDefaultClose();
});
$('create-form').addEventListener('submit',async event=>{
  event.preventDefault(); if (busy || !state || !state.local_rehearsal_available) return;
  rehearsalError(''); lock(true); $('rehearsal-status').textContent='Creating a new local auction and admission authority…';
  try {
    const cutoff=Math.floor(new Date($('create-close').value).getTime()/1000);
    if (!Number.isSafeInteger(cutoff) || cutoff<Date.now()/1000+30 || cutoff>Date.now()/1000+86400) throw new Error('Choose a close time between 30 seconds and 24 hours from now.');
    const result=await rehearsalRequest('create',{units:integer('create-units'),reserve:integer('create-reserve'),budget:integer('create-budget'),closes_at_utc:cutoff});
    showRoom(result); history.replaceState(null,'','#auction='+room.auction_id);
    $('rehearsal-status').textContent='✓ Auction created. Share the local link and seal a demo bid.';
  } catch (error) { rehearsalError(error.message); $('rehearsal-status').textContent=''; }
  finally { lock(false); refreshRoom(); }
});
$('bid-form').addEventListener('submit',async event=>{
  event.preventDefault(); if (busy || !state || !room) return;
  rehearsalError(''); lock(true); $('rehearsal-status').textContent='Generating an input ZK proof and three REP3 share pairs, then verifying admission…';
  try {
    const maximum=integer('maximum-bid'); $('maximum-bid').value='';
    const result=await rehearsalRequest('bid',{auction_id:room.auction_id,maximum_bid:maximum});
    if (!result.accepted || !result.client_input_proof_verified || result.share_pairs_verified!==3 || result.receipt_signatures_verified!==3 || result.chain_funding_verified || result.native_settlement_started || result.auction.auction_id!==room.auction_id) throw new Error('Incomplete signed acceptance.');
    lastAcceptance=result; showRoom(result.auction);
    $('bid-accepted').hidden=false; $('bid-accepted').replaceChildren(element('h3','✓ Bid accepted'),element('p','Ticket '+String(result.ticket_number).padStart(2,'0')+' · Signed acceptance verified.'),element('p','Your maximum bid stays out of the public acceptance. Synthetic funding only.'));
    $('copy-acceptance').hidden=false; $('acceptance-details').hidden=false;
    $('acceptance-public').textContent=JSON.stringify({client_input_proof_verified:true,share_pairs_verified:3,receipt_signatures_verified:3,ticket:result.ticket,commitment:result.commitment,receipt:result.receipt},null,2);
    $('rehearsal-status').textContent='✓ Fresh signed acceptance ready. '+result.seconds.toFixed(1)+' seconds.';
  } catch (error) {
    rehearsalError(error.message); $('rehearsal-status').textContent='';
    try {
      const response=await fetch('/api/rehearsal/auction/'+room.auction_id);
      if (response.ok) showRoom(await response.json());
    } catch { /* Keep the original admission error visible. */ }
  }
  finally { lock(false); refreshRoom(); }
});
$('close-auction').addEventListener('click',async()=>{
  if (busy || !room) return; rehearsalError(''); lock(true);
  try {
    const result=await rehearsalRequest('close',{auction_id:room.auction_id});
    if (!result.closed || result.funding_scope!=='offline-synthetic' || result.fresh_clearing_or_settlement) throw new Error('Unexpected book certification.');
    room.closed=true; room.accepted_root=result.accepted_root;
    $('rehearsal-status').textContent='✓ Accepted book certified. Root: '+result.accepted_root;
  } catch(error) { rehearsalError(error.message); }
  finally { lock(false); refreshRoom(); }
});
$('copy-auction-link').addEventListener('click',async()=>{
  try { await navigator.clipboard.writeText($('auction-link').value); $('rehearsal-status').textContent='✓ Local auction link copied.'; }
  catch { $('auction-link').select(); $('rehearsal-status').textContent='Select and copy the local link.'; }
});
$('copy-acceptance').addEventListener('click',async()=>{
  if (!lastAcceptance) return;
  const data={format:1,scope:'offline-synthetic browser rehearsal; no chain funding or settlement',auction:lastAcceptance.auction,receipt:lastAcceptance.receipt};
  const json=JSON.stringify(data,null,2);
  try {
    await navigator.clipboard.writeText(json);
    $('rehearsal-status').textContent='✓ Public signed acceptance copied. No bid opening or key included.';
  } catch {
    $('acceptance-details').open=true; $('acceptance-public').textContent=json;
    $('rehearsal-status').textContent='Select and copy the public acceptance below. No bid opening or key included.';
  }
});
async function joinRehearsal() {
  const match=/^#auction=([0-9a-f]{32})$/.exec(location.hash); if (!match || !state) return;
  try {
    const response=await fetch('/api/rehearsal/auction/'+match[1]); const value=await response.json();
    if (!response.ok) throw new Error(value.error || 'Local auction unavailable.');
    showRoom(value); $('rehearsal').hidden=false; $('recorded-demo').hidden=true;
  } catch (error) { $('rehearsal').hidden=false; rehearsalError(error.message); }
}
window.addEventListener('hashchange',joinRehearsal);
function setDefaultClose() { const close=new Date(Date.now()+300000); $('create-close').value=new Date(close-close.getTimezoneOffset()*60000).toISOString().slice(0,19); }
setDefaultClose();
setInterval(refreshRoom,1000);
