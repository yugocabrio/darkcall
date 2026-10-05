'use strict';
const $ = id => document.getElementById(id);
let state, selected = 'positive', busy = false;
const setError = message => { $('error').textContent = message; $('error').hidden = !message; };
function element(tag, text, className) { const node = document.createElement(tag); node.textContent = text; if (className) node.className = className; return node; }
function resetResult() {
  const result = $('result'); result.className = 'result-box'; result.replaceChildren(
    element('span','◈','result-symbol'), element('h3','Still sealed to you.'),
    element('p','Verify the proof, decrypt your memo, then open your own result.'));
  ['proof','native','open'].forEach(step => $('step-'+step).classList.remove('done'));
  $('verify').replaceChildren(document.createTextNode('Verify & open my result '), element('span','↗'));
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
function lock(value) { busy = value; document.querySelectorAll('button,select').forEach(node => node.disabled = value); }
async function run(action) {
  if (busy || !state) return;
  const ticket = Number($('ticket').value);
  lock(true); setError('');
  if (action === 'verify') {
    resetResult(); $('result').className = 'result-box loading';
    $('result').replaceChildren(element('span','◈','result-symbol'),element('h3','Checking your receipt…'),element('p','Running the fixed proof verifier and native decryption on this computer.'));
  } else $('tamper-result').textContent = 'Submitting a corrupted proof to the real verifier…';
  try {
    const response = await fetch('/api/verify', {method:'POST',headers:{'Content-Type':'application/json','X-DarkCall-Token':state.csrf},body:JSON.stringify({case:selected,ticket,action})});
    const result = await response.json(); if (!response.ok) throw new Error(result.error || 'Verification failed');
    if (action === 'tamper') {
      if (!result.rejected || result.key_read || result.memo_decrypted) throw new Error('Unexpected tamper result');
      $('tamper-result').textContent = '✓ Changed proof rejected. Recipient key untouched. Memo stays encrypted.';
    } else {
      if (!result.fixed_cpp_ticket_proof_verified || !result.native_proof_and_signatures_verified || !result.own_ticket_opening_verified || !result.native_amount_matches_refund || result.ticket_index !== ticket) throw new Error('Incomplete verification result');
      const box = $('result'); box.className = 'result-box verified';
      box.replaceChildren(element('span','✓','result-symbol'),element('h3',result.won ? 'You won a unit.' : 'Your bid was not filled.'),element('p','Your own memo matches the verified receipt proof.'));
      const amounts = element('div','','receipt-amounts');
      for (const [label,amount] of [['Your refund',result.refund],['Clearing price',result.price]]) {
        const column = element('div',''); column.append(element('span',label+' · zatoshi'),element('strong',String(amount))); amounts.append(column);
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
lock(true);
fetch('/api/state').then(async response => { const data=await response.json(); if (!response.ok) throw new Error(data.error || 'Demo unavailable'); state=data; renderCase(); lock(false); }).catch(error => setError(error.message));
