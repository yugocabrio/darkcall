'use strict';
const $ = id => document.getElementById(id);
let state, selected = 'positive', busy = false, receiptVerified = false, settlementVerified = false;
function stages() {
  $('stage-status').textContent = (receiptVerified ? 'Clearing and your result verified. ' : 'Recorded clearing; own result unopened. ') + (settlementVerified ? 'Shielded settlement re-verified in this session.' : 'Settlement verified when recorded; re-verify it here.');
  const checks = $('result-checks'); checks.replaceChildren();
  if (receiptVerified) checks.append(element('span','✓ Auction & receipt verified'),element('span','✓ Native Zcash transaction'));
  if (settlementVerified && state.six_output_memo_e2e) checks.append(element('span','✓ Settlement verified'));
  checks.hidden = !checks.childElementCount;
}
const setError = message => { $('error').textContent = message; $('error').hidden = !message; };
const bidderName = n => 'Bidder '+String(n).padStart(2,'0');
const labelOf = i => ((state.cases.find(c => c.id === selected) || state.cases[0]).labels || [1,2,3,4])[i];
// A role emoji (the same set as the walkthrough), hidden from screen readers.
const ICONS={bidder:'🧑🏻',operator:'🖥️',escrow:'🔒',public:'🌐',key:'🔑'};
function icon(name) { const node=document.createElement('span'); node.className='icon'; node.setAttribute('aria-hidden','true'); node.textContent=ICONS[name]; return node; }
function element(tag, text, className) { const node = document.createElement(tag); node.textContent = text; if (className) node.className = className; return node; }
function resetResult() {
  receiptVerified = false; stages();
  // Start as the public: what anyone can see, and what stays sealed.
  const result = $('result'); result.className = 'result-box'; const sees=element('ul','','public-view');
  for (const line of ['The clearing price and the units sold','Four sealed tickets, no amounts','One shielded transaction with six outputs']) sees.append(element('li',line));
  const hidden=element('ul','','public-view sealed-view');
  for (const line of ['Who won','Any bid','Any refund']) hidden.append(element('li',line));
  const title=element('h3','You are the public.'); title.prepend(icon('public'));
  result.replaceChildren(title, element('p','You can see:'), sees, element('p','You cannot see:'), hidden);
  ['proof','native','open'].forEach(step => $('step-'+step).classList.remove('done'));
  $('verify').textContent='Switch to '+$('ticket').selectedOptions[0].textContent+"'s view";
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
  } else { $('cheat-detail').hidden=true; $('tamper-result').textContent = 'Changing one byte of a bidder\'s receipt proof and giving it to the real verifier…'; }
  try {
    const response = await fetch('/api/verify', {method:'POST',headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:JSON.stringify({case:selected,ticket,action})});
    const result = await response.json(); if (!response.ok) throw new Error(result.error || 'Verification failed');
    if (action === 'tamper') {
      if (!result.rejected || result.key_read || result.memo_decrypted) throw new Error('Unexpected tamper result');
      $('tamper-result').replaceChildren(element('b','✗ Refused. '),document.createTextNode('The fixed verifier rejects the corrupted receipt proof before the bidder\'s key is read, so the receipt stays encrypted.'));
    } else {
      if (!result.fixed_cpp_ticket_proof_verified || !result.native_proof_and_signatures_verified || !result.own_ticket_opening_verified || !result.native_amount_matches_refund || result.ticket_index !== ticket) throw new Error('Incomplete verification result');
      const current = state.cases.find(c => c.id === selected);
      if (result.txid !== current.txid || result.price !== current.price || result.sold !== current.sold || (state.six_output_memo_e2e && (!result.receipt_in_six_output_transaction || !result.fixed_auction_context_proof_verified))) throw new Error('Receipt belongs to a different auction');
      receiptVerified = true; stages();
      const box = $('result'); box.className = 'result-box verified';
      box.replaceChildren(element('h3',result.won ? 'You won a unit' : 'No unit this time'),element('p',result.won ? 'At '+result.price+' zatoshi, the price every winner pays.' : 'Your full refund is verified.'));
      const amounts = element('div','','receipt-amounts');
      for (const [label,amount] of [['Refund (zatoshi)',result.refund]]) {
        const column = element('div',''); column.append(element('span',label),element('strong',String(amount))); amounts.append(column);
      }
      box.append(amounts);
      const who=bidderName(labelOf(ticket));
      const mine=element('div','','contrast-box'), mineTitle=element('b','What '+who+' sees'); mineTitle.prepend(icon('key')); mine.append(mineTitle,element('p',(result.won?'Won a unit at '+result.price:'No unit')+' · refund '+result.refund+' zatoshi, decrypted with '+who+"'s own key."));
      const pub=element('div','','contrast-box'), pubTitle=element('b','What everyone else sees'); pubTitle.prepend(icon('public')); pub.append(pubTitle,element('p','One shielded transaction with six outputs: no amounts, no addresses, no memo text.'));
      if (state.recorded_chain==='testnet') pub.lastChild.append(document.createTextNode(' '),Object.assign(document.createElement('a'),{href:'https://testnet.zecblock.com/tx/'+result.txid,textContent:'Open it on the explorer',rel:'noopener'}));
      const contrast=element('div','','contrast'); contrast.append(mine,pub); box.append(contrast);
      ['proof','native','open'].forEach(step => $('step-'+step).classList.add('done'));
      $('verify').textContent='Verify again';
    }
  } catch (error) { if (action === 'verify') resetResult(); else $('tamper-result').textContent=''; setError(error.message); }
  finally { $('result').classList.remove('loading'); lock(false); }
}
document.querySelectorAll('[data-case]').forEach(button => button.addEventListener('click',() => { if (!busy && state) { selected=button.dataset.case; renderCase(); } }));
$('ticket').addEventListener('change',resetResult);
$('verify').addEventListener('click',() => run('verify'));
// Cheat lab: each cheat changes a copy of the recorded settlement one way and
// reports which of the four claims the real verifiers say it breaks.
const CLAIMS={price:'The price is right',payouts:'Every payout matches the result',book:'These are the accepted bids',transaction:'The Zcash transaction is valid',bundle:'The package check'};
const CHECK={book:'the signed-book check',price:'the auction proofs',payouts:'the output bindings'};
const WHY={book:'The changed bid no longer hashes to the accepted root that all three operators signed and that the proofs were made over.',
  price:'The auction proofs were made for the true price; they do not verify for any other.',
  payout:'Each payout proof is bound to one output of the transaction and one row of the result; checked against another output, it fails.'};
async function cheat(kind) {
  if (busy || !state) return;
  lock(true); setError('');
  $('cheat-detail').hidden=true;
  $('tamper-result').textContent='Copying this settlement with that one change and running the real verifiers on it'+(kind==='book'?'…':'. About 2 minutes…');
  try {
    const response=await fetch('/api/cheat',{method:'POST',headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:JSON.stringify({kind})});
    const result=await response.json(); if (!response.ok) throw new Error(result.error || 'Cheat check failed');
    if (result.outcome==='accepted') throw new Error('The verifiers accepted a changed settlement. Please report this.');
    if (result.outcome!=='refused') { $('tamper-result').textContent='This test did not run as intended ('+(result.test_error||'no targeted refusal')+'), so it is not counted as a refusal.'; return; }
    const broken=result.caught_by.map(c=>'“'+(CLAIMS[c]||c)+'”').join(' and ');
    $('tamper-result').replaceChildren(element('b','✗ Refused in '+Math.round(result.seconds)+' s. '),
      document.createTextNode(broken+' fails. '+(WHY[kind]||'')));
    showCheatDetail(result);
  } catch (error) { $('tamper-result').textContent=''; setError(error.message); }
  finally { lock(false); }
}
// Original bundle beside the changed copy, then exactly what changed.
function showCheatDetail(result) {
  const box=(cls,title,text)=>{ const b=element('div','','cheat-side '+cls); b.append(element('b',title),element('p',text)); return b; };
  const sides=element('div','','cheat-compare');
  sides.append(box('ok','Original bundle','✓ Accepted. The same verifiers pass it: the unchanged control copy and the recorded check.'),
    box('no','Your changed copy','✗ Rejected in '+Math.round(result.seconds)+' s by '+(CHECK[result.expected_check]||'the verifier')+(result.verifier_message?': “'+result.verifier_message+'”':'')+'.'));
  const c=result.changed||{}, more=document.createElement('details'), list=element('div','','cheat-diff');
  more.append(element('summary','What exactly changed'));
  const who=Number.isInteger(c.ticket_index)?'Ticket '+(c.ticket_index+1)+' in the public book · ':'';
  for (const [label,value] of [['File',(c.files||[]).join(', ')],['Field',who+(c.field||'')],['Before',c.before],['After',c.after]]) {
    if (!value) continue; const row=element('div',''); row.append(element('span',label),element('code',value)); list.append(row);
  }
  more.append(list,element('p','The bundle in the repository is not touched. The change is made to a working copy whose bundle.json is re-sealed with the new file digests, so the package check passes and the refusal comes from the verifier itself.','cheat-note'));
  $('cheat-detail').replaceChildren(sides,more); $('cheat-detail').hidden=false;
}
document.querySelectorAll('[data-cheat]').forEach(button=>button.addEventListener('click',()=>cheat(button.dataset.cheat)));
$('tamper').addEventListener('click',() => run('tamper'));
// The settlement check, said as the four things it proves.
function claim(ok,title,text,extra) {
  const row=element('div','','claim '+(ok?'claim-ok':'claim-no'));
  const body=element('div',''); body.append(element('b',title),document.createTextNode(' '+text)); if (extra) body.append(extra);
  row.append(element('span',ok?'✓':'✗','claim-mark'),body); return row;
}
function showClaims(checks) {
  const price=state.cases[0].price, labels=[...(state.cases[0].labels||[1,2,3,4])].sort((a,b)=>a-b);
  const rows=element('div','','claim-rows');
  for (const label of [...labels.map(n=>'Refund · '+bidderName(n)),'Seller','Operator fee']) rows.append(element('span',label+(checks.payouts?' ✓':' ✗')));
  const tx=state.cases[0].txid, explorer=state.recorded_chain==='testnet'?Object.assign(document.createElement('a'),{href:'https://testnet.zecblock.com/tx/'+tx,textContent:'See it on the explorer',rel:'noopener'}):null;
  const note=element('p','Not part of this check: whether the transaction is in the chain. ','claim-note'); if (explorer) note.append(explorer,document.createTextNode('.'));
  $('settlement-claims').replaceChildren(
    claim(checks.price,'The price is right.',price+' zatoshi follows the auction rule over exactly the four sealed bids. This check needs no bid, and the proof reveals none.'),
    claim(checks.payouts,'Every payout matches the result.','Each shielded output pays the amount and recipient the result says:',rows),
    claim(checks.transaction,'The Zcash transaction is valid.','One valid shielded Zcash transaction, signed from the escrow by 2 of the 3 operators together; no single operator can sign.'),
    claim(checks.book,'These are the accepted bids.','All three operators signed this exact book of four sealed bids.'),
    note);
  $('settlement-claims').hidden=false; $('settlement-claims').classList.remove('pending');
}
// The proof trail: each claim's public data, with the consistency checks that
// tie one file to another and to the transaction bytes (recomputed by the
// server from the committed bundle). Values show shortened; click one for all
// of it, and each line links to its file in the repository.
const REPO='https://github.com/yugocabrio/ZECATHON/blob/main/';
const shortValue = v => /^(0x)?[0-9a-f]{24,}$/i.test(v) ? v.slice(0,12)+'…'+v.slice(-8) : v;
function mark(ok) { return element('span',ok?'✓':'✗',ok?'trail-ok':'trail-no'); }
function value(name, full) {
  const wrap=element('span','','trail-value'), code=element('code',shortValue(String(full)));
  if (shortValue(String(full))===String(full)) { code.className='plain'; if (name) wrap.append(document.createTextNode(name+' ')); wrap.append(code); return wrap; }
  code.title='Click to show all of it'; code.tabIndex=0;
  const toggle=()=>{ code.textContent=code.textContent===String(full)?shortValue(String(full)):String(full); };
  code.addEventListener('click',toggle); code.addEventListener('keydown',e=>{ if (e.key==='Enter') toggle(); });
  if (name) wrap.append(document.createTextNode(name+' ')); wrap.append(code); return wrap;
}
function trailLine(label, items, file, ...checks) {
  const row=element('div','','trail-line'), values=element('div','','trail-values');
  for (const [name,full] of items) values.append(value(name,full));
  if (file && trailBundle) values.append(Object.assign(document.createElement('a'),{href:REPO+trailBundle+'/'+file,textContent:file+' ↗',className:'trail-file',rel:'noopener'}));
  const c=element('div','','trail-checks'); for (const [ok,text] of checks) c.append(mark(ok),document.createTextNode(' '+text+' '));
  row.append(element('span',label,'trail-label'),values,c); return row;
}
function trailSection(title, why, ...lines) {
  const box=element('section','','trail-section'); box.append(element('h4',title),element('p',why,'trail-why'),...lines); return box;
}
let trailBundle=null;
function showTrail(t) {
  trailBundle=t.bundle.path;
  const b=t.book, p=t.price, x=t.transaction, kb=n=>n.toLocaleString()+' bytes';
  const book=trailSection('These are the accepted bids','The auction proof takes the four bid commitments and their root as public inputs, and all three operators signed the same root. Change one bid and they no longer agree: try it below. Tickets are listed in public-book order (sorted by ticket id).',
    ...b.commitments.map(c=>trailLine('Ticket '+c.ticket+' commitment',[['',c.commitment]],b.file,[c.in_auction_proof,'in the auction proof\'s public inputs'])),
    trailLine('Accepted root',[['',b.accepted_root]],b.file,[b.root_in_auction_proof,'auction proof'],[b.root_in_receipt_proof,'receipt proof']),
    ...b.operators.map(o=>trailLine('Operator '+o.operator,[['signature',o.signature],['trusted key',o.trusted_key]],o.file,[o.signed_root_matches,'signed this root'],[o.signed_commitments_match,'signed the same four bids'])));
  const price=trailSection('The price is right','These are public inputs of the auction proof. It verifies only for exactly these values, so a changed price or terms are refused: try it below.',
    trailLine('Price · sold',[['',p.price+' zatoshi · '+p.sold+' of '+p.units]],p.file,[p.receipt_proof_same_price_and_sold,'same in the receipt proof']),
    trailLine('Terms',[['','reserve '+p.reserve+' · budget '+p.budget+' · fee '+p.processing_fee.toLocaleString()]],p.file,[p.terms_match_authority,'match the trusted authority']),
    trailLine('Auction proof',[[kb(p.proof.bytes)+' · sha256',p.proof.sha256]],p.proof.file),
    trailLine('Verifying key',[[p.public_inputs+' public inputs · sha256',p.verifying_key.sha256]],p.verifying_key.file),
    trailLine('Proof system',[['',p.proof_system]]));
  const payouts=trailSection('Every payout matches the result','Each row hash is an output of the auction proof. Its Halo2 proof shows that the transaction note at that action pays exactly that row\'s amount to that row\'s recipient, without revealing either. Which output pays which ticket is public; the amount and recipient are not.',
    ...t.payouts.map(r=>trailLine(r.payee,[['row hash',r.row_hash],['action',String(r.action_index)],['cmx',r.cmx],['Halo2 proof sha256',r.halo2_proof.sha256]],r.file,
      [r.row_hash_in_auction_proof,'row hash in the auction proof'],[r.action_index_in_auction_proof,'action in the auction proof'],[r.cmx_in_transaction_bytes_at_that_action,'cmx read from the transaction bytes at that action'])));
  const txLines=[trailLine('Transaction',[['txid',x.txid],['sha256',x.sha256]],x.file,[x.txid_from_bytes_matches_bundle,'txid computed from the bytes'],[x.actions_from_bytes_match_bundle,'six cmx read from the bytes']),
    trailLine('Shape',[['','NU7 branch '+x.branch+' · '+kb(x.bytes)+' · '+x.actions+' actions']])];
  if (x.recorded_block) txLines.push(trailLine('On chain (recorded)',[['','block '+x.recorded_block.height.toLocaleString()+' · '+x.recorded_block.confirmations+' confirmations, observed through '+x.recorded_block.observed_through+'; not proven by the bundle']]));
  const tx=trailSection('The Zcash transaction is valid','The verifier checks the transaction\'s own proof and its 2-of-3 escrow signature. Here its txid and note commitments are read from the bytes and compared with the bundle. Whether it is on chain is a separate observation, shown last.',...txLines);
  const sections=[book,price,payouts,tx], count=sections.reduce((n,s)=>n+s.querySelectorAll('.trail-ok,.trail-no').length,0);
  const intro=element('p','','trail-intro');
  intro.append(mark(t.bundle.all_digests_match_bundle_json),document.createTextNode(' Read from the committed public bundle ('+t.bundle.files+' files, each matching its digest in bundle.json) and the trusted authority, never from private files. ✓ marks one of '+(count+1)+' consistency checks between these files and the transaction bytes, recomputed when this page loads. They are not proofs: the proofs and signatures are checked by Re-run verification.'));
  const how=element('p','','trail-intro'); how.append(document.createTextNode('Rebuild it yourself: '),element('code','python3 -B scripts/proof_trail.py '+t.bundle.path+' '+t.bundle.authority));
  $('trail-body').replaceChildren(intro,...sections,how); $('trail').hidden=false;
}
const shortDate = iso => new Date(iso+'T12:00:00Z').toLocaleDateString('en-US',{month:'short',day:'numeric',timeZone:'UTC'});
function showRecordedClaims() {
  const recorded=state.cases[0].recorded_check; if (!recorded) return false;
  const record=Object.assign(document.createElement('a'),{href:'https://github.com/yugocabrio/ZECATHON/blob/main/'+recorded.record,textContent:'record',rel:'noopener'});
  $('settlement-result').replaceChildren(element('b','Recorded: '),document.createTextNode('verified on '+shortDate(recorded.date)+' in a clean Linux container built from the committed source ('),record,document.createTextNode('). What that proves:'));
  showClaims(recorded); return true;
}
$('settlement').addEventListener('click', async () => {
  if (busy || !state) return;
  lock(true); setError('');
  $('settlement-result').textContent = 'Running the real public verifier on this server. About 100 seconds…';
  $('settlement-claims').classList.add('pending');
  try {
    const response = await fetch('/api/settlement', {method:'POST', headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:'{}'});
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || 'Settlement verification failed');
    if (!result.native_row_proofs_verified || !result.native_proof_and_signatures_verified || !result.normal_cpp_auction_proof_verified || !result.actual_transaction_outputs_match || result.private_openings_read || result.chain_funding_or_finality_verified) throw new Error('Incomplete settlement verification');
    if (state.six_output_memo_e2e && (result.txid !== state.cases[0].txid || !result.receipt_fixed_proof_verified || !result.receipt_settlement_context_link_verified)) throw new Error('Settlement belongs to a different receipt auction');
    settlementVerified = true; stages();
    $('settlement-result').replaceChildren(element('b','Verified again just now: '),document.createTextNode('in '+Math.round(result.seconds)+' s on this server, with no viewing key or other secret. What that proves:'));
    showClaims({price:result.normal_cpp_auction_proof_verified, payouts:result.native_row_proofs_verified && result.actual_transaction_outputs_match,
      transaction:result.native_proof_and_signatures_verified, book:Boolean(result.acceptance_or_recipient_receipts_authenticated)});
    $('settlement-detailed-result').textContent = 'Native authorization, six bindings, fixed auction/receipt proofs and context verified. ' + (result.acceptance_or_recipient_receipts_authenticated ? 'Accepted-book signatures verified. ' : 'No funded admission certificate for this synthetic run. ') + 'Transaction: '+result.txid;
  } catch (error) { if (!showRecordedClaims()) { $('settlement-result').textContent=''; $('settlement-claims').hidden=true; } setError(error.message); }
  finally { lock(false); }
});
lock(true);
fetch('/api/state').then(async response => {
  const data=await response.json(); if (!response.ok) throw new Error(data.error || 'Demo unavailable'); state=data;
  const recordedTestnet=state.recorded_chain==='testnet';
  // Bidders in label order; the value stays the ticket's place in the public book.
  const labels=state.cases[0].labels||[1,2,3,4];
  $('ticket').replaceChildren(...labels.map((n,i)=>[n,i]).sort((a,b)=>a[0]-b[0]).map(([n,i])=>Object.assign(document.createElement('option'),{value:String(i),textContent:bidderName(n)})));
  if (state.cases[0].labels_in_sealing_order) document.querySelector('.receipt-panel .panel-lead').append(' Numbered in the order the bids were sealed'+(recordedTestnet?', as in the video.':'.'));
  $('proof-card-scope').textContent=state.funded_canonical?(recordedTestnet?'Recorded on public Zcash testnet':'Recorded auction · isolated regtest'):'Separate example · synthetic transactions';
  if (recordedTestnet) $('hero-chain').textContent='Zcash NU7 · public testnet';
  if (state.hosted) {
    for (const walker=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT); walker.nextNode();) {
      const node=walker.currentNode, text=node.nodeValue.replace(/on this computer/g,'on this server').replace(/one local test wallet/g,'one test wallet on this server');
      if (text!==node.nodeValue) node.nodeValue=text;
    }
    // Hosted: the bidder client runs on this server, so say so wherever a bid is typed.
    $('client-scope').textContent="On this hosted demo each bidder's client runs on the server, so the server sees the demo bid you type; operators receive only secret shares of each bid and proofs. In a deployment the client runs on the bidder's own device.";
    $('bid-client-scope').textContent='On this hosted demo your bid client runs on the server, which sees the demo bid you type; operators get only secret shares of it and a proof.';
    $('share-scope').textContent='Open this link in another browser to follow the same auction.';
  }
  if (state.six_output_memo_e2e) {
    $('case-tabs').hidden = true;
    $('scope-description').textContent = 'Recorded proof-backed demo · synthetic funding, no broadcast.';
    $('native-shape').textContent = 'Ironwood · NU7 · 6 outputs / 4 encrypted receipts';
    $('settlement-scope').textContent = 'Verify that the shielded Zcash transaction matches the auction outcome, without seeing the bids. About 100 seconds.';
    $('remaining-scope').textContent = 'Synthetic funding and no broadcast in this view; all parties on one host. Funded runs on public testnet and regtest are served by ./demo.sh.';
    if (state.funded_canonical) {
      $('scope-description').textContent = state.recorded_note || 'Recorded proof-backed demo on isolated Zcash regtest.';
      if (recordedTestnet) {
        // Already verified when recorded; the button re-runs the same check here.
        $('settlement-scope').textContent='Check that the shielded Zcash transaction matches the auction outcome, without seeing any bid. The result below is recorded; run the same check again here in about 100 seconds, with no viewing key or other secret.';
        $('settlement').textContent='Re-run verification';
      }
      $('remaining-scope').textContent = 'This recorded auction is separate from the auctions you create now; any auction you create under Run an auction can be settled the same way with Settle on Zcash. Independent hosts and larger bid counts remain unverified.';
      $('chain-scope').textContent = recordedTestnet ? 'Two public-testnet confirmations recorded through one trusted lightwalletd provider; the public bundle does not itself prove chain inclusion.' : state.recorded_local_confirmations ? 'Two controlled regtest confirmations recorded; public bundle does not prove chain inclusion' : 'Recorded regtest run; no chain inclusion proof in the public bundle';
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
  const recordedShown = state.funded_canonical && showRecordedClaims();
  if (state.funded_canonical) fetch('/api/trail').then(r=>r.ok?r.json():null).then(t=>{ if (t) showTrail(t); }).catch(()=>{});
  if (state.last_settlement && !recordedShown) {
    $('settlement-result').textContent = 'Previous public check passed. Run again to reverify.';
    $('settlement-detailed-result').textContent = 'Previous public verification passed for transaction: '+state.last_settlement.txid+'. Not reverified in this page session.';
  }
  lock(false);
  if (onTestnet()) {
    $('local-pill').textContent=(state.hosted?'Live demo':'Local demo')+' · new auctions settle on public Zcash testnet';
    for (const id of ['fund-chain','settle-chain']) $(id).textContent='Public testnet';
    for (const node of document.querySelectorAll('.chain-text')) node.textContent='public Zcash testnet';
    // Measured on public testnet, Oct 19: funding 612 s (a 600 s admission
    // window), settlement 984-1122 s, about 30 min from creation to receipts.
    $('try-card-scope').textContent='Real shielded settlement · about 30 min on testnet';
    $('run-time').textContent='About 30 minutes on public testnet, most of it funding and settlement.';
    $('fund-time').textContent='About 10 minutes on public testnet, most of it the admission window.';
    $('settle-time').textContent='About 16 to 19 minutes on public testnet, including two confirmations.';
  }
  const recordedTx=state.cases[0].txid, hero=state.cases[0];
  $('cheat-price-label').textContent='Claim the price was '+(hero.price+1);
  // What is sold and what the refunds mean, from the served terms.
  const words=['no','one','two','three','four'], losers=hero.tickets-hero.sold;
  if (Number.isInteger(hero.budget) && hero.sold>0 && hero.sold<=hero.units) $('auction-story').textContent='Think of a newsletter selling '+words[hero.units]+' sponsor slots this month. Each sponsor locked a budget of '+hero.budget+' zatoshi, plus a fixed fee. The '+words[hero.sold]+' highest sponsors each got a slot at the same price, '+hero.price+' zatoshi, and '+(hero.budget-hero.price)+' back'+(losers?'; the other '+(losers===1?'sponsor':words[losers]+' sponsors')+' got all '+hero.budget+' back':'')+'. No operator ever rebuilds the bid book, not even to prove the price, so next month\'s floor can\'t be set from the winners\' ceilings; only the clearing price is published, and by the auction rule it is the highest losing bid.';
  $('hero-price').textContent=hero.price; $('hero-tickets').textContent=hero.tickets+' sealed bids';
  $('hero-sold').textContent=hero.sold+' of '+hero.units+' units sold · every winner pays the same';
  $('hero-txid').textContent=recordedTx.slice(0,8)+'…'+recordedTx.slice(-8); $('hero-txid').title=recordedTx;
  if (state.recorded_chain==='testnet') $('hero-txlink').href='https://testnet.zecblock.com/tx/'+recordedTx;
  $('hero-note').textContent=state.funded_canonical ? (state.recorded_note || 'Recorded on isolated Zcash regtest.') : 'Recorded example with synthetic funding, not broadcast.';
  if (!state.funded_canonical) $('hero-label').textContent='Recorded example';
  // The recorded run's own note, then its settlement transaction (an explorer link on testnet).
  const txShort=recordedTx.slice(0,8)+'…'+recordedTx.slice(-8);
  const txNode=state.recorded_chain==='testnet'?Object.assign(document.createElement('a'),{href:'https://testnet.zecblock.com/tx/'+recordedTx,textContent:txShort,rel:'noopener'}):document.createTextNode(txShort);
  $('scope-description').replaceChildren(($('scope-description').textContent||'')+' · settlement tx ',txNode);
  refreshRoom(); route();
}).catch(error => setError(error.message));

// Interactive entry is an OFFLINE rehearsal. It never alters the recorded book.
let room = null, lastAcceptance = null, freshClearingVerified = false, freshResultVerified = false, bookFolded = false;
// Every ticket sealed in THIS page keeps its own local opening capability. The page
// simulates four bidders; each opening uses only that ticket's capability and key.
let myTickets = [], selectedTicket = 0;
const currentTicket = () => myTickets[selectedTicket] || null;
function renderTicketPickers() {
  for (const id of ['my-ticket','my-native-ticket']) {
    const select=$(id), label=select.closest('label');
    label.hidden=myTickets.length<2;
    if (select.options.length!==myTickets.length) select.replaceChildren(...myTickets.map((t,i)=>{ const option=document.createElement('option'); option.value=String(i); option.textContent='Ticket '+String(t.ticket_number).padStart(2,'0'); return option; }));
    select.value=String(selectedTicket); select.disabled=busy;
  }
}
function pickTicket(value) {
  selectedTicket=Number(value); freshResultVerified=false; nativeResultVerified=false;
  $('fresh-result').className='result-box'; $('fresh-result').replaceChildren(element('h3','Still sealed.'),element('p','Open this ticket\'s result.'));
  $('native-result').className='result-box'; $('native-result').replaceChildren(element('h3','Your receipt is encrypted.'),element('p','Only this ticket\'s viewing key can open its memo.'));
  $('fresh-tamper-result').textContent=''; $('native-tamper-result').textContent='';
  refreshRoom();
}
const integer = id => { const text=$(id).value; if (!/^\d+$/.test(text)) throw new Error('Use whole-number demo values.'); return Number(text); };
function rehearsalError(message) { $('rehearsal-error').textContent=message; $('rehearsal-error').hidden=!message; if (message) $('rehearsal-error').scrollIntoView({behavior:'smooth',block:'center'}); }
// Bring the next primary action into view after a step completes.
function focusNext(id) { const node=$(id); if (node && !node.hidden) setTimeout(()=>node.scrollIntoView({behavior:'smooth',block:'center'}),150); }
function renderSlots() {
  const slots=$('sealed-slots'); if (!room) { slots.replaceChildren(); return; }
  slots.replaceChildren(...[0,1,2,3].map(i=>{ const filled=i<room.accepted; const node=element('span',''); node.className=filled?'filled':'';
    node.append(document.createTextNode('Ticket '+String(i+1).padStart(2,'0')),element('b',''));
    node.setAttribute('aria-label','Ticket '+String(i+1).padStart(2,'0')+(filled?' sealed':' open')); return node; }));
}
function refreshRoom() {
  const available=Boolean(state && state.local_rehearsal_available);
  $('create-auction').disabled=busy || !available;
  $('new-auction').hidden=!room;
  // Funding and settlement run on the server for minutes; leaving would drop the
  // ticket capabilities this page holds, so the button waits for the job.
  $('new-auction').disabled=busy || jobRunning();
  const settled=Boolean(room && room.settlement && room.settlement.state==='done');
  const funded=Boolean(room && room.funding && room.funding.state==='done');
  const complete=[Boolean(room),Boolean(room && room.accepted===4),funded || Boolean(room && !room.funding_required && freshClearingVerified),freshClearingVerified,freshResultVerified || settled,settled];
  const current=complete.findIndex(done=>!done);
  ['create','bid','fund','clear','result','settle'].forEach((name,i)=>{
    const step=$('progress-'+name); step.classList.toggle('done',complete[i]);
    if (i===current) step.setAttribute('aria-current','step'); else step.removeAttribute('aria-current');
  });
  if (!room) return;
  const remaining=Math.max(0,room.closes_at_utc-Math.floor(Date.now()/1000));
  $('room-terms').textContent=room.units+' units · reserve '+room.reserve+' · budget '+room.budget+' zatoshi';
  $('room-count').textContent=room.accepted+' / 4 sealed';
  renderSlots();
  $('maximum-bid').max=room.budget;
  $('seal-bid').disabled=busy || !available || room.stopped_after_incomplete_admission || remaining===0 || room.accepted>=4 || room.closed;
  $('close-auction').disabled=busy || !available || room.stopped_after_incomplete_admission || remaining>0 || room.accepted!==4 || room.closed;
  const fundingPending=Boolean(room.funding_required) && !(room.funding && room.funding.state==='done');
  $('clear-auction').disabled=busy || !available || !room.clearing_available || !room.closed || room.clearing_stopped || Boolean(room.clearing) || fundingPending;
  const settling=Boolean(room.settlement && room.settlement.state==='running') || Boolean(room.funding && room.funding.state==='running');
  $('open-fresh-result').disabled=busy || settling || !room.clearing || !currentTicket();
  $('tamper-fresh').disabled=busy || settling || !room.clearing || !currentTicket();
  renderTicketPickers(); renderFunding();
  // Clearing is shown once it is the next real step: after close, and after funding when required.
  $('clearing-section').hidden=!room.closed || fundingPending;
  renderSettlement();
  $('clear-auction').hidden=Boolean(room.clearing);
  $('rehearsal-title').textContent=settled?'Your auction settled on Zcash.':room.clearing?'Your auction cleared.':room.closed?(fundingPending?'Fund the sealed book.':'Ready to clear.'):room.accepted===4?'Bids are sealed.':'Place your bid.';
  document.querySelector('.rehearsal-scope').textContent=settled?'Same sealed book. One shielded transaction. Publicly verified.':room.clearing?'Fresh outcome. Verified against your accepted bids.':room.closed?(fundingPending?'Every sealed ticket gets a real shielded deposit before clearing.':'Four sealed bids. One verified outcome.'):room.accepted===4?'Waiting for the scheduled close.':'Seal four demo bids. Each gets a ZK proof and three operator signatures.';
  $('close-status').textContent=room.stopped_after_incomplete_admission ? 'Admission stopped. The checkpoint is preserved.' : room.closed ? '✓ Closed bids certified.' :
    remaining>0 ? 'Bidding closes in '+Math.floor(remaining/60)+':'+String(remaining%60).padStart(2,'0')+'.' :
    room.accepted===4 ? 'Deadline reached. Confirm the accepted bids.' : 'Closed. Four tickets are required.';
}
function showRoom(value) {
  if (!value || value.funding_scope!=='offline-synthetic' || value.chain_funding_verified || value.native_settlement_started!==Boolean(value.settlement)) throw new Error('Unexpected rehearsal scope.');
  if (room && room.auction_id!==value.auction_id) {
    lastAcceptance=null; myTickets=[]; selectedTicket=0; $('bid-accepted').hidden=true; $('copy-acceptance').hidden=true; $('acceptance-details').hidden=true;
    resetFresh();
  }
  room=value; $('create-form').hidden=true; $('auction-room').hidden=false;
  $('auction-link').value=location.origin+'/#auction='+room.auction_id;
  refreshRoom();
  if (room.clearing) renderFresh(room.clearing);
  if (room.closed && !bookFolded) { bookFolded=true; $('book-details').open=false; }
}
async function rehearsalRequest(action,payload) {
  const response=await fetch('/api/rehearsal/'+action,{method:'POST',headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:JSON.stringify(payload)});
  const result=await response.json(); if (!response.ok) throw new Error(result.error || 'Local rehearsal stopped.'); return result;
}
function resetFresh() {
  $('rehearsal-label').textContent='Demo bids'; fundedFocused=false; settledFocused=false; bookFolded=false;
  for (const id of ['clearing-section','fund-section','settle-section']) $(id).hidden=true;
  freshClearingVerified=false; freshResultVerified=false; nativeResultVerified=false; $('book-details').open=true;
  $('native-result').className='result-box'; $('native-result').replaceChildren(element('h3','Your receipt is encrypted.'),element('p','Only your viewing key can open your memo in this transaction.'));
  $('native-tamper-result').textContent='';
  $('clearing-title').textContent='Discover the price.';
  $('fresh-outcome').hidden=true; $('clearing-status').textContent=''; $('fresh-tamper-result').textContent='';
  $('fresh-result').className='result-box';
  $('fresh-result').replaceChildren(element('h3','Still sealed.'),element('p','Open the ticket accepted in this browser session.'));
}
function renderFresh(result) {
  if (!room || result.auction_id!==room.auction_id || result.accepted_root!==room.accepted_root
      || !result.rep3_clearing_verified || !result.fixed_cpp_proof_verified || !result.certified_book_matches
      || result.certificate_signatures_verified!==3 || result.native_settlement) throw new Error('Clearing does not match this certified book.');
  freshClearingVerified=true;
  $('fresh-outcome').hidden=false;
  $('clearing-title').textContent='One price. Your result.';
  $('fresh-price').textContent=result.price;
  $('fresh-sold').textContent=result.sold+' / '+room.units;
  $('clearing-status').textContent='✓ Auction verified · '+result.seconds.toFixed(1)+'s';
  $('fresh-proof-details').textContent='REP3 delivery_flags4/v2 · fixed C++ verification · three final-book signatures. Proof generation: '+result.proof_generation_seconds.toFixed(1)+' seconds.';
  $('fresh-root').textContent='Same accepted root: '+result.accepted_root;
}
$('clear-auction').addEventListener('click',async()=>{
  if (busy || !room) return; rehearsalError(''); lock(true);
  $('clearing-status').textContent='Clearing these secret shares and generating a fresh MPC proof…';
  try {
    const result=await rehearsalRequest('clear',{auction_id:room.auction_id});
    renderFresh(result); room.clearing=result; room.fresh_clearing_verified=true; focusNext('open-fresh-result');
    $('rehearsal-status').textContent='';
    $('book-details').open=false;
  } catch(error) {
    $('clearing-status').textContent=''; rehearsalError(error.message);
    try { const response=await fetch('/api/rehearsal/auction/'+room.auction_id); if (response.ok) showRoom(await response.json()); } catch { /* Preserve failure. */ }
  } finally { lock(false); refreshRoom(); }
});
async function openFresh(action) {
  const mine=currentTicket(); if (busy || !room || !mine || !room.clearing) return;
  rehearsalError(''); lock(true);
  if (action==='open') { freshResultVerified=false; $('fresh-result').replaceChildren(element('h3','Verifying…'),element('p','Checking your proof and signed book.')); }
  else $('fresh-tamper-result').textContent='Submitting a corrupted clearing proof to the fixed verifier…';
  try {
    const result=await rehearsalRequest('result',{auction_id:room.auction_id,ticket:mine.ticket,opening_token:mine.opening_token,action});
    if (result.auction_id!==room.auction_id || result.ticket!==mine.ticket || result.native_settlement) throw new Error('Unexpected result scope.');
    if (action==='tamper') {
      if (!result.rejected || result.private_pad_read) throw new Error('Unexpected tamper check.');
      $('fresh-tamper-result').textContent='✓ Corrupted proof rejected. Your ticket pads were not read.';
    } else {
      if (!result.fixed_cpp_proof_verified || !result.certified_book_matches || result.accepted_root!==room.accepted_root
          || result.price!==room.clearing.price || result.sold!==room.clearing.sold || !result.opening_verified) throw new Error('Incomplete own-result verification.');
      const box=$('fresh-result'); box.className='result-box verified';
      freshResultVerified=true;
      box.replaceChildren(element('h3',result.won?'You won a unit':'No unit this time'),element('p',result.won?'At '+result.price+' zatoshi, the price every winner pays.':'No unit allocated. Your outcome is verified.'));
      const amounts=element('div','','receipt-amounts'), column=element('div','');
      column.append(element('span','Refund due (zatoshi)'),element('strong',String(result.refund))); amounts.append(column); box.append(amounts,element('p','✓ Your result verified'));
      if (room.settlement_available && !room.settlement) focusNext('settle-auction');
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
// Same-book settlement: every state comes from the server's recorded driver progress.
// [step, begins at phase:state, ends at phase:state]; later phases also finish earlier steps.
const SETTLE_STEPS=[['mpc','settle_preflight:started','binding_key:started'],
  ['bindings','binding_key:started','mpc_verification:started'],['sign','mpc_verification:started','settlement:done'],
  ['verify','public_verification:started','public_verification:done']];
let settlePoll=null, nativeResultVerified=false, fundedFocused=false, settledFocused=false;
const shortId=value=>value?value.slice(0,10)+'…'+value.slice(-8):'—';
function minutes(seconds) { seconds=Math.max(0,Math.round(seconds)); return Math.floor(seconds/60)+':'+String(seconds%60).padStart(2,'0'); }
// Where Fund and Settle run: the owned regtest, or public testnet when the
// server was started with --chain testnet.
const onTestnet=()=>Boolean(state && state.live_chain==='testnet');
const chainName=()=>onTestnet()?'Zcash testnet':'regtest';
function renderSettlement() {
  const section=$('settle-section'), job=room && room.settlement, settledState=Boolean(job && job.state==='done');
  $('rehearsal-label').textContent=settledState?'Settled on '+chainName():job?'Settling on '+chainName():'Demo bids';
  const fundedState=Boolean(room && room.funding && room.funding.state==='done');
  $('payment-status').textContent=settledState?'Paid in one shielded '+(onTestnet()?'testnet':'regtest')+' transaction ↓':job?'Settlement on '+chainName()+' in progress ↓':fundedState?'Real deposits locked · paid out at Settle ↓':'No deposit yet · funding comes after the close';
  section.hidden=!(room && room.clearing);
  if (section.hidden) return;
  const available=Boolean(room.settlement_available);
  $('settle-auction').hidden=Boolean(job);
  $('settle-auction').disabled=busy || !available || Boolean(job);
  $('settle-note').hidden=available || Boolean(job);
  $('settle-note').textContent=room.units===3 && room.budget===100 && [0,20].includes(room.reserve) ? (onTestnet()?'Testnet':'Regtest')+' settlement tools are not available on this checkout.' : 'The native settlement prototype supports the default terms: 3 units, budget 100, reserve 0 or 20.';
  $('settle-steps').hidden=!job;
  if (!job) { $('settle-status').textContent=''; $('settle-outcome').hidden=true; return; }
  const events=job.events||[], at={};
  for (const e of events) { const key=e.phase+':'+e.state; if (!(key in at)) at[key]=e.at_unix; }
  const rows=events.filter(e=>/^binding_row_\d$/.test(e.phase)).length;
  const now=Math.floor(Date.now()/1000), failed=job.state==='failed';
  SETTLE_STEPS.forEach(([id,begin,end],i)=>{
    const li=$('settle-step-'+id);
    const finished=end in at || SETTLE_STEPS.slice(i+1).some(([,later])=>later in at) && id!=='verify';
    const active=!finished && begin in at;
    li.className=finished?'done':active?(failed?'failed':'running'):'';
    let label='';
    if (finished && begin in at) label=minutes((at[end]||SETTLE_STEPS.slice(i+1).map(([,later])=>at[later]).find(Boolean)||now)-at[begin]);
    if (active) { label=failed?'stopped':minutes(now-at[begin]); if (id==='bindings' && rows) label='row '+rows+'/6 · '+label; }
    li.querySelector('em').textContent=label;
  });
  const total=minutes(now-job.started_at_unix);
  if (room.funding && room.funding.result) $('settle-deposit').textContent=room.funding.result.deposit_txid;
  const result=job.result;
  $('settle-status').textContent=failed?(job.error||'Settlement stopped.'):job.state==='done'?'✓ Settled and publicly verified · '+minutes(result.settlement_seconds+result.public_verification_seconds)+' of proving and checks':'Running · '+total+' elapsed · other proofs wait until this finishes';
  $('settle-outcome').hidden=job.state!=='done';
  if (job.state==='done') {
    if (!settledFocused) { settledFocused=true; focusNext('settle-txid'); }
    if (result.accepted_root!==room.accepted_root || result.price!==room.clearing.price || result.sold!==room.clearing.sold) { $('settle-status').textContent='Settlement result does not match this auction.'; $('settle-outcome').hidden=true; return; }
    $('settle-txid').textContent=result.txid; $('settle-deposit').textContent=result.deposit_txid;
    $('know-public').textContent='Price '+result.price+' zatoshi, '+result.sold+' of '+room.units+' sold, one shielded transaction and its proofs.';
    $('settle-root').textContent='✓ '+shortId(result.accepted_root)+' · same tickets and accepted root as your browser book';
    $('settle-price').textContent=result.price+' zatoshi × '+result.sold+' sold · matches the browser clearing · '+result.confirmations+(onTestnet()?' public testnet':' regtest')+' confirmations';
    $('open-native-result').disabled=busy || !currentTicket();
    $('tamper-native').disabled=busy || !currentTicket();
    if (!currentTicket() && !nativeResultVerified) $('native-result').replaceChildren(element('h3','Your receipt is encrypted.'),element('p','This page no longer holds a ticket capability. Reloading loses it by design.'));
  }
  if (job.state==='running' && !settlePoll) settlePoll=setTimeout(pollSettlement,3000);
}
function renderFunding() {
  const section=$('fund-section'), job=room && room.funding;
  section.hidden=!(room && room.funding_required && room.closed);
  if (section.hidden) return;
  $('fund-auction').hidden=Boolean(job);
  $('fund-auction').disabled=busy || Boolean(job);
  $('fund-steps').hidden=!job;
  $('fund-facts').hidden=!(job && job.state==='done');
  if (!job) { $('fund-status').textContent='Clearing waits until the deposits are admitted.'; return; }
  const at={}; for (const e of job.events||[]) { const key=e.phase+':'+e.state; if (!(key in at)) at[key]=e.at_unix; }
  const now=Math.floor(Date.now()/1000), li=$('fund-step-deposit');
  const done=job.state==='done', failed=job.state==='failed';
  li.className=done?'done':failed?'failed':'running';
  const begin=at['preflight:started']||job.started_at_unix;
  li.querySelector('em').textContent=done?minutes((at['funding:done']||now)-begin):failed?'stopped':minutes(now-begin);
  $('fund-title').textContent=done?'Deposits locked.':'Lock the deposits.';
  $('fund-status').textContent=failed?(job.error||'Funding stopped.'):done?'✓ Funded and admitted on '+chainName()+'. Now clear the book.':'Running · '+minutes(now-job.started_at_unix)+' elapsed · includes a '+(onTestnet()?'10-minute':'120 s')+' admission window';
  if (done && job.result) {
    if (job.result.accepted_root!==room.accepted_root) { $('fund-status').textContent='Funding does not match this book.'; return; }
    if (!fundedFocused) { fundedFocused=true; focusNext('clear-auction'); }
    $('fund-deposit').textContent=job.result.deposit_txid;
    $('fund-admission').textContent='✓ '+job.result.client_proofs+' bid proofs + '+job.result.share_pairs+' secret-share pairs re-verified · '+job.result.receipt_signatures+' receipts · '+job.result.final_book_signatures+' book signatures · same root '+shortId(job.result.accepted_root);
  }
  if (job.state==='running' && !settlePoll) settlePoll=setTimeout(pollSettlement,3000);
}
async function pollSettlement() {
  settlePoll=null; if (!room) return;
  try { const response=await fetch('/api/rehearsal/auction/'+room.auction_id); if (response.ok) showRoom(await response.json()); }
  catch { /* Retry on the next tick. */ }
  if (room && ((room.settlement && room.settlement.state==='running') || (room.funding && room.funding.state==='running')) && !settlePoll) settlePoll=setTimeout(pollSettlement,3000);
}
$('fund-auction').addEventListener('click',async()=>{
  if (busy || !room || room.funding) return; rehearsalError(''); lock(true);
  try { room.funding=await rehearsalRequest('fund',{auction_id:room.auction_id}); $('rehearsal-status').textContent=''; }
  catch(error) { rehearsalError(error.message); }
  finally { lock(false); refreshRoom(); }
});
$('settle-auction').addEventListener('click',async()=>{
  if (busy || !room || room.settlement) return; rehearsalError(''); lock(true);
  try {
    const view=await rehearsalRequest('settle',{auction_id:room.auction_id});
    room.settlement=view; room.native_settlement_started=true;
  } catch(error) { rehearsalError(error.message); }
  finally { lock(false); refreshRoom(); }
});
async function openNative(action) {
  const mine=currentTicket(); if (busy || !room || !mine || !room.settlement || room.settlement.state!=='done') return;
  rehearsalError(''); lock(true);
  if (action==='native-open') { nativeResultVerified=false; $('native-result').className='result-box loading'; $('native-result').replaceChildren(element('h3','Decrypting…'),element('p','Verifying the proofs and signed book first, then using only your viewing key.')); }
  else $('native-tamper-result').textContent='Submitting a corrupted receipt proof to the fixed verifier…';
  try {
    const result=await rehearsalRequest('result',{auction_id:room.auction_id,ticket:mine.ticket,opening_token:mine.opening_token,action});
    if (result.auction_id!==room.auction_id || result.ticket!==mine.ticket) throw new Error('Unexpected receipt scope.');
    if (action==='native-tamper') {
      if (!result.rejected || result.key_read || result.memo_decrypted) throw new Error('Unexpected tamper check.');
      $('native-tamper-result').textContent='✓ Corrupted receipt proof rejected before your viewing key was read.';
    } else {
      const job=room.settlement.result;
      if (!result.native_settlement || result.txid!==job.txid || result.price!==job.price || result.sold!==job.sold || !result.fixed_cpp_ticket_and_auction_proofs_verified
          || !result.accepted_book_and_recipients_authenticated || !result.own_ticket_opening_verified || !result.native_amount_matches_refund
          || !result.native_proof_and_signatures_verified || !result.own_viewing_key_only || !result.receipt_in_same_six_output_transaction) throw new Error('Incomplete shielded receipt verification.');
      nativeResultVerified=true;
      const box=$('native-result'); box.className='result-box verified';
      box.replaceChildren(element('h3',result.won?'You won a unit':'No unit this time'),element('p',result.won?'At '+result.price+' zatoshi, paid from your deposit':'No unit allocated · full budget refunded'));
      const amounts=element('div','','receipt-amounts'), column=element('div','');
      column.append(element('span','Refund received (zatoshi)'),element('strong',String(result.refund))); amounts.append(column);
      box.append(amounts,element('p','✓ Decrypted from '+shortId(result.txid)+' with your viewing key only'));
    }
  } catch(error) {
    if (action==='native-open') { $('native-result').className='result-box'; $('native-result').replaceChildren(element('h3','Opening stopped.'),element('p','No receipt was opened.')); }
    else $('native-tamper-result').textContent='';
    rehearsalError(error.message);
  } finally { $('native-result').classList.remove('loading'); lock(false); refreshRoom(); }
}
$('open-native-result').addEventListener('click',()=>openNative('native-open'));
$('tamper-native').addEventListener('click',()=>openNative('native-tamper'));
$('open-fresh-result').addEventListener('click',()=>openFresh('open'));
$('my-ticket').addEventListener('change',event=>pickTicket(event.target.value));
$('my-native-ticket').addEventListener('change',event=>pickTicket(event.target.value));
$('tamper-fresh').addEventListener('click',()=>openFresh('tamper'));
function startAuction() {
  selectExperience('try');
  $('rehearsal').hidden=false; $('recorded-demo').hidden=true;
  if (!room) setDefaultClose();
  $('rehearsal-status').textContent=state && state.local_rehearsal_available ? '' : 'Local input prover and admission helper are required; recorded verification is available below.';
  refreshRoom(); $('rehearsal').scrollIntoView({behavior:'smooth',block:'start'});
}
$('start-auction').addEventListener('click',startAuction);
function showRecording() {
  selectExperience('settlement');
  $('rehearsal').hidden=true; $('recorded-demo').hidden=false;
  // Never use the new maximum bid or receipt to choose an old fixture identity.
  $('recorded-demo').scrollIntoView({behavior:'smooth',block:'start'});
}
// Each live-demo mode has a shareable path; the landing is '/'.
const MODES={'/demo/settled':'settlement','/demo/run':'try'};
function selectExperience(mode) {
  const path=mode==='try'?'/demo/run':'/demo/settled';
  if (location.pathname!==path) history.pushState(null,'',path+(mode==='try'&&room?'#auction='+room.auction_id:''));
  document.body.classList.add('has-experience');
  for (const [id,name] of [['start-auction','try'],['show-recording','settlement']]) {
    $(id).classList.toggle('active',mode===name); $(id).setAttribute('aria-pressed',String(mode===name));
  }
}
// Leave either mode without reloading: a running auction stays on the server
// and "Run an auction" reopens it.
function showOverview(push=true) {
  if (push && location.pathname!=='/') history.pushState(null,'','/');
  document.body.classList.remove('has-experience');
  $('rehearsal').hidden=true; $('recorded-demo').hidden=true;
  for (const id of ['start-auction','show-recording']) { $(id).classList.remove('active'); $(id).setAttribute('aria-pressed','false'); }
  window.scrollTo({top:0,behavior:'smooth'});
}
$('back-overview').addEventListener('click',()=>showOverview());
function route() {
  if (/^#auction=/.test(location.hash)) return joinRehearsal();
  const mode=MODES[location.pathname];
  if (mode==='settlement') showRecording(); else if (mode==='try') startAuction();
  else if (document.body.classList.contains('has-experience')) showOverview(false);
}
window.addEventListener('popstate',()=>{ if (state) route(); });
$('show-recording').addEventListener('click',showRecording);
// The end of the landing offers the same two ways in.
document.querySelectorAll('[data-go]').forEach(button=>button.addEventListener('click',()=>(button.dataset.go==='try'?startAuction:showRecording)()));
$('continue-recording').addEventListener('click',showRecording);
function jobRunning() {
  return Boolean(room && ((room.funding && room.funding.state==='running') || (room.settlement && room.settlement.state==='running')));
}
$('new-auction').addEventListener('click',()=>{
  if (busy || jobRunning()) return;
  // Ticket capabilities live only in this page. Dropping them before the
  // receipts are opened makes those receipts unopenable, so ask first.
  if (myTickets.length && !nativeResultVerified
      && !window.confirm('Start another auction? This page holds the only opening capability for your tickets in this auction; without it you cannot open their receipts.')) return;
  room=null; lastAcceptance=null; myTickets=[]; selectedTicket=0; history.replaceState(null,'',location.pathname);
  resetFresh();
  $('auction-room').hidden=true; $('create-form').hidden=false; $('rehearsal-title').textContent='Create your auction.';
  document.querySelector('.rehearsal-scope').textContent='Set the terms. Invite four bidders.';
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
    $('rehearsal-status').textContent='✓ Auction created. Seal a demo bid; the auction link opens it in another browser.';
    focusNext('maximum-bid');
  } catch (error) { rehearsalError(error.message); $('rehearsal-status').textContent=''; }
  finally { lock(false); refreshRoom(); }
});
$('bid-form').addEventListener('submit',async event=>{
  event.preventDefault(); if (busy || !state || !room) return;
  rehearsalError(''); lock(true); $('rehearsal-status').textContent='Generating an input ZK proof and splitting the bid into REP3 secret shares, a pair for each operator, then verifying admission…';
  try {
    const maximum=integer('maximum-bid'); $('maximum-bid').value='';
    const result=await rehearsalRequest('bid',{auction_id:room.auction_id,maximum_bid:maximum});
    if (!result.accepted || !result.client_input_proof_verified || result.share_pairs_verified!==3 || result.receipt_signatures_verified!==3 || result.chain_funding_verified || result.native_settlement_started || result.auction.auction_id!==room.auction_id) throw new Error('Incomplete signed acceptance.');
    lastAcceptance=result; showRoom(result.auction);
    myTickets.push({ticket:result.ticket,opening_token:result.opening_token,ticket_number:result.ticket_number}); selectedTicket=myTickets.length-1;
    $('bid-accepted').hidden=false; $('bid-accepted').replaceChildren(element('h3','Bid sealed'),element('p','Ticket '+String(result.ticket_number).padStart(2,'0')+' · proof, 3 secret-share pairs (one per operator) and 3 operator signatures verified'+(room.funding_required?' · deposit after close':'')+'.'));
    $('copy-acceptance').hidden=false; $('acceptance-details').hidden=false;
    $('acceptance-public').textContent=JSON.stringify({client_input_proof_verified:true,share_pairs_verified:3,receipt_signatures_verified:3,ticket:result.ticket,commitment:result.commitment,receipt:result.receipt},null,2);
    $('rehearsal-status').textContent='✓ Bid sealed and pre-checked in '+result.seconds.toFixed(1)+' seconds.';
    if (room.accepted===4) focusNext('close-auction');
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
    $('rehearsal-status').textContent=room.funding_required?'✓ Bids locked. Fund them on Zcash next.':'✓ Bids locked. Ready to clear.';
    $('book-details').open=false; // Bidding is over: fold the bid form away from the next step.
    refreshRoom(); focusNext(room.funding_required?'fund-auction':'clear-auction');
  } catch(error) { rehearsalError(error.message); }
  finally { lock(false); refreshRoom(); }
});
document.querySelectorAll('[data-bid]').forEach(button=>button.addEventListener('click',()=>{ $('maximum-bid').value=button.dataset.bid; $('maximum-bid').focus(); }));
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
    showRoom(value); selectExperience('try'); $('rehearsal').hidden=false; $('recorded-demo').hidden=true;
  } catch (error) { $('rehearsal').hidden=false; rehearsalError(error.message); }
}
window.addEventListener('hashchange',joinRehearsal);
// Ticket capabilities live only in this page by design; warn before a reload throws them away.
window.addEventListener('beforeunload',event=>{
  if (myTickets.length && room && !nativeResultVerified) { event.preventDefault(); event.returnValue=''; }
});
function setDefaultClose() { const close=new Date(Date.now()+120000); $('create-close').value=new Date(close-close.getTimezoneOffset()*60000).toISOString().slice(0,19); }
setDefaultClose();
setInterval(refreshRoom,1000);

// "Follow one auction": a static walkthrough of the demo bids 77, 61, 43, 21.
// Values are shown only when the reader asks; the protocol never publishes them.
(function walkthrough() {
  const stage=$('walk-stage'); if (!stage) return;
  const captions=[
    [['Seal. ',1],['The bidder\'s own client turns each bid into a commitment and three secret shares, {1}{2}{3}, and sends them to the operators. Operator A receives {1}{3}, B {2}{1}, C {3}{2}: ',0],['no operator ever holds all three, so none can read a bid',1],['. A zero-knowledge proof ties every commitment to its shares.',0]],
    [['Fund. ',1],['Each ticket locks 15,100 zatoshi, its 100 budget plus a 15,000 processing fee for the network and operator fees, in a ',0],['shielded 2-of-3 escrow',1],['. The operators check the deposits against the same proofs and secret shares, then all three ',0],['sign the accepted book',1],['. Its key was made jointly by DKG, so no operator holds it whole; any two of the three must sign together (FROST) to pay out.',0]],
    [['Prove. ',1],['The three operators compute together on their secret shares and generate one proof jointly: multi-party ZK proving with coNoir. With the demo bids 77, 61, 43 and 21 and three units, the top three win and each pays the ',0],['price of 21',1],[', the highest losing bid. The proof shows it follows the rules over exactly the committed bids; ',0],['no bid list is ever rebuilt',1],['.',0]],
    [['Settle. ',1],['The same computation yields six payment rows. Each winner gets its 100 budget back minus the price, 79; the loser gets all 100; the seller gets 3 × 21 = 63; the operator gets a 30,000 fee. The other 30,000 deposited pays the network fee. ',0],['Six Halo2 proofs bind each row to a real note',1],[' in one shielded transaction. Once every proof checks out, ',0],['two of the three operators sign it together with FROST',1],[': each adds a signature share, so no single operator can move the escrow.',0]],
    [['Verify. ',1],['Anyone checks the proofs ',0],['with no secrets and no viewing key',1],[': price 21, three sold, every output bound to the result. Ticket 01 decrypts its own memo with its own key: ',0],['won, refund 79',1],['.',0]],
  ];
  let step=1, timer=null; const sealTimers={};
  // Show the demo values for a moment, then let the mask close over them.
  function sealing(name) {
    clearTimeout(sealTimers[name]); stage.classList.add(name);
    sealTimers[name]=setTimeout(()=>stage.classList.remove(name),1200);
  }
  function show(n) {
    step=n; stage.dataset.step=String(n);
    if (n===1) sealing('open-bids');
    if (n===4) { stage.classList.add('binding'); void stage.offsetWidth; requestAnimationFrame(()=>stage.classList.remove('binding')); }
    for (const node of stage.querySelectorAll('.w-from')) {
      node.classList.toggle('on',Number(node.dataset.from)<=n);
      node.classList.toggle('now',(node.dataset.now||node.dataset.from).split(' ').map(Number).includes(n));
    }
    for (const button of document.querySelectorAll('.walk-steps button')) {
      if (Number(button.dataset.step)===n) button.setAttribute('aria-current','step'); else button.removeAttribute('aria-current');
    }
    // {1}{2}{3} in a caption are drawn as the same share chips as the stage.
    const withShares = text => text.split(/((?:\{[123]\})+)/).map((part,i)=>{ if (!(i%2)) return document.createTextNode(part); const group=element('i','','shares'); group.append(...[...part.matchAll(/[123]/g)].map(([n])=>element('i',n,'share s'+n))); return group; });
    $('walk-caption').replaceChildren(...captions[n-1].map(([text,bold])=>{ if (!bold) return withShares(text); const b=element('b',''); b.append(...withShares(text)); return b; }).flat());
  }
  function stop() { if (timer) { clearInterval(timer); timer=null; $('walk-play').textContent='▶ Play'; } }
  for (const button of document.querySelectorAll('.walk-steps button')) button.addEventListener('click',()=>{ stop(); show(Number(button.dataset.step)); });
  $('walk-play').addEventListener('click',()=>{
    if (timer) { stop(); return; }
    if (step===5) show(1);
    $('walk-play').textContent='❚❚ Pause';
    timer=setInterval(()=>{ if (step>=5) stop(); else show(step+1); },3600);
  });
  // Seals: tap to read one (pointing works on desktop through :hover).
  for (const seal of stage.querySelectorAll('.w-seal')) seal.addEventListener('click',()=>seal.classList.toggle('peek'));
  // The first time the stage scrolls into view, show the bids being sealed.
  if ('IntersectionObserver' in window) {
    const seen=new IntersectionObserver(entries=>{ if (entries.some(e=>e.isIntersecting)) { seen.disconnect(); if (step===1) sealing('open-bids'); } },{threshold:.6});
    seen.observe(stage.querySelector('.w-bidders'));
  }
  show(1);
})();
