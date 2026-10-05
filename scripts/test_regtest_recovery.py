"""Crash points around send, mining and three durable outcome records."""
import unittest
from unittest.mock import patch
import regtest_recovery as recovery
from regtest_node import RpcError


class RecoveryTests(unittest.TestCase):
    binary = b'example-signed-bytes'
    txid = '01' * 32

    def operators(self, phase='Prepared'):
        return [{'operator': i, 'phase': phase, 'accepted_tickets': 4,
                 'reserved_transaction_matches': True} for i in range(3)]

    def observed(self, confirmations=2):
        return {'txid': self.txid, 'hex': self.binary.hex(),
                'confirmations': confirmations, 'in_active_chain': confirmations > 0}

    def reconcile(self, observations, operators=None, tip='original'):
        mutations = []
        observed = iter(observations)
        def rpc(method, params=None, timeout=None):
            mutations.append((method, params))
            if method == 'sendrawtransaction':
                return self.txid
            if method == 'getrawmempool':
                return [self.txid]
            if method != 'generate':
                raise AssertionError('Unexpected RPC method: ' + method)
        with patch.object(recovery, 'status', return_value={'tip': tip}), \
             patch.object(recovery, 'lookup_transaction', side_effect=lambda _: next(observed)), \
             patch.object(recovery, 'rpc', side_effect=rpc), \
             patch.object(recovery, 'call', return_value={'journal_events_written': 0}) as record:
            result = recovery.reconcile_transaction(self.binary, self.txid, 'admission',
                'original', self.operators() if operators is None else operators)
        return result, mutations, record

    def test_confirmed_transaction_after_crash_is_neither_resent_nor_remined(self):
        _, mutations, record = self.reconcile([self.observed(), self.observed()],
                                              self.operators('Finalized'), tip='later')
        self.assertEqual(mutations, [])
        record.assert_called_once_with(recovery.ADMIT, 'record', 'admission', self.txid, 'broadcast')

    def test_missing_transaction_is_sent_exactly_once_and_mined(self):
        _, mutations, _ = self.reconcile([None, self.observed()])
        self.assertEqual(mutations, [('sendrawtransaction', [self.binary.hex()]), ('generate', [2])])

    def test_mempool_crash_and_one_confirmation_require_only_remaining_blocks(self):
        _, mutations, _ = self.reconcile([self.observed(0), self.observed()])
        self.assertEqual(mutations, [('getrawmempool', None), ('generate', [2])])
        _, mutations, _ = self.reconcile([self.observed(1), self.observed()])
        self.assertEqual(mutations, [('generate', [1])])

    def test_changed_chain_or_reserved_digest_prevents_send_and_record(self):
        with self.assertRaisesRegex(RuntimeError, 'chain changed'):
            self.reconcile([None], tip='changed')
        operators = self.operators()
        operators[2]['reserved_transaction_matches'] = False
        with patch.object(recovery, 'rpc') as rpc, patch.object(recovery, 'call') as record:
            with self.assertRaisesRegex(RuntimeError, 'same exact reserved'):
                recovery.reconcile_transaction(self.binary, self.txid, 'admission', 'original', operators)
            rpc.assert_not_called()
            record.assert_not_called()

    def test_reorg_or_changed_bytes_are_not_treated_as_pending(self):
        for value in ({**self.observed(), 'in_active_chain': False},
                      {**self.observed(), 'hex': '00'},
                      {**self.observed(0), 'blockhash': '02' * 32},
                      {**self.observed(), 'confirmations': -1}):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                self.reconcile([value])
        with self.assertRaisesRegex(RuntimeError, 'lost confirmations'):
            self.reconcile([self.observed(1)], self.operators('Finalized'))

    def test_only_explicit_node_not_found_means_unbroadcast(self):
        with patch.object(recovery, 'rpc', side_effect=RpcError({'code': -5, 'message': 'not found'})):
            self.assertIsNone(recovery.lookup_transaction(self.txid))
        for error in (RpcError({'code': -28, 'message': 'warming up'}),
                      ConnectionError('node unavailable')):
            with patch.object(recovery, 'rpc', side_effect=error), self.assertRaises(type(error)):
                recovery.lookup_transaction(self.txid)

    def test_capture_restart_compares_entire_actual_node_capture(self):
        # Higher-level capture equivalence and real SDK replay are also checked
        # on the retained regtest fixture; no cached plaintext-only scan here.
        import json
        import pathlib
        import tempfile
        import regtest_deposits as deposits
        with tempfile.TemporaryDirectory() as directory:
            wallet = pathlib.Path(directory)
            snapshot = {'blocks': 3, 'tip': 'final', 'consensus': {'chaintip': '37a5165b'}}
            initial = {'height': 2, 'hash': 'initial', 'ironwood_tree': ''}
            final = {'height': 3, 'hash': 'final', 'ironwood': {'commitments': {'finalState': ''}}}
            block = {'previousblockhash': 'initial', 'time': 1,
                     'trees': {'ironwood': {'size': 0}}, 'tx': []}
            def rpc(method, params):
                return 'final' if method == 'getblockhash' else block if method == 'getblock' else final
            with patch.object(deposits, 'rpc', side_effect=rpc), \
                 patch.object(deposits, 'status', return_value={'tip': 'final'}), \
                 patch.object(deposits, 'call', return_value={'checked': True}), \
                 patch.object(deposits, 'private_read', side_effect=lambda p, _: p.read_bytes()):
                a = deposits.capture(wallet, initial, snapshot)
                b = deposits.capture(wallet, initial, snapshot)
                self.assertEqual(a, b)
                p = wallet / 'node-capture-3.json'
                changed = json.loads(p.read_text())
                changed['blocks'][0]['prev_hash'] = 'changed'
                p.write_text(json.dumps(changed))
                with self.assertRaisesRegex(RuntimeError, 'differs from current canonical'):
                    deposits.capture(wallet, initial, snapshot)


if __name__ == '__main__':
    unittest.main()
