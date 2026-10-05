"""A copied crash-point prefix must be exact, bounded by its original Prepare."""
import json
import unittest
from regtest_prepared_gate import journal_prefix


class PrefixTests(unittest.TestCase):
    def fixture(self, cancellation=False):
        return b''.join(json.dumps(v).encode() + b'\n' for v in (
            {'header': 'only a fixture'},
            {'sequence': 1, 'action': {'event': 'Freeze'}},
            {'sequence': 2, 'action': {'event': 'LockFinalBook', 'digest': [0] * 32}},
            {'sequence': 3, 'action': {'event': 'Prepare', 'effects': [1] * 32, 'cancellation': cancellation}},
            {'sequence': 4, 'action': {'event': 'Broadcast', 'effects': [1] * 32}},
            {'sequence': 5, 'action': {'event': 'Finalize', 'effects': [1] * 32}},
        ))

    def test_exact_prefix_excludes_later_outcomes_and_never_changes_source(self):
        original = self.fixture()
        lines = original.splitlines(keepends=True)
        self.assertEqual(journal_prefix(original, True), b''.join(lines[:4]))
        self.assertEqual(journal_prefix(original, False), b''.join(lines[:3]))
        self.assertEqual(original, self.fixture())

    def test_truncated_or_out_of_order_source_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'truncated'):
            journal_prefix(self.fixture()[:-1], True)
        with self.assertRaisesRegex(RuntimeError, 'sequence'):
            journal_prefix(self.fixture().replace(b'"sequence": 2', b'"sequence": 3'), True)

    def test_cancellation_is_not_a_settlement_crash_fixture(self):
        with self.assertRaisesRegex(RuntimeError, 'settlement reservation'):
            journal_prefix(self.fixture(True), True)

    def test_missing_prepare_or_outcome_before_prepare_is_rejected(self):
        lines = self.fixture().splitlines(keepends=True)
        with self.assertRaisesRegex(RuntimeError, 'lacks'):
            journal_prefix(b''.join(lines[:3]), True)
        with self.assertRaisesRegex(RuntimeError, 'preceding'):
            journal_prefix(self.fixture().replace(b'"event": "Prepare"', b'"event": "Broadcast"'), True)


if __name__ == '__main__':
    unittest.main()
