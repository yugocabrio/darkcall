#!/usr/bin/env python3
"""Stage times of one funded settlement, from the files it already writes.

Usage: python3 -B scripts/settlement_timings.py SETTLEMENT_RUN [E2E_RESULT.json]
SETTLEMENT_RUN is the native run folder (.local/settle-...-settlement); the
optional E2E result is what scripts/check_fresh_settlement.py wrote. Reads
timings and flags only: no proof, key, pad or opening is read or printed.
"""
import json
import pathlib
import sys


def load(path):
    return json.loads(pathlib.Path(path).read_text())


def timings(run, e2e=None):
    run = pathlib.Path(run)
    native = load(run / 'native-proof-summary.json')
    construction = load(run / 'construction-summary.json')
    confirmed = load(run / 'confirmed-summary.json')
    gate = load(pathlib.Path(load(run / 'mpc-result.json')['run']) / 'summary.json')
    stages = {stage['name']: round(stage['seconds'], 1) for stage in gate['stages']}
    rows = [round(t, 1) for t in native['row_proof_and_verify_seconds']]
    result = {
        'run': run.name,
        'price': native['price'], 'sold': native['sold'],
        'halo2_key_seconds': round(native['key_seconds'], 1),
        'halo2_row_proof_and_verify_seconds': rows,
        'halo2_rows_total_seconds': round(sum(native['row_proof_and_verify_seconds']), 1),
        'row_workers': native.get('row_workers', 1),
        'native_proving_seconds': round(native['seconds'], 1),
        'mpc_gate_seconds': round(sum(s['seconds'] for s in gate['stages']), 1),
        'mpc_gate_row_reverification_seconds': stages.get('native-proof-verification'),
        'mpc_settlement_proof_seconds': stages.get('proof'),
        'orchard_proof_seconds': round(construction['native']['proof_seconds'], 1),
        'pre_signing_replay_seconds': round(construction['native']['application']['seconds'], 1),
        'construction_seconds': round(construction['seconds'], 1),
        'driver_seconds_with_confirmations_and_scans': round(confirmed['seconds'], 1),
        'all_six_rows_verified': native['proof_verified'] and native['rows'] == 6,
        'pre_signing_replay_verified': construction['native']['application']['native_row_proofs_verified'],
    }
    if e2e:
        e2e = load(e2e)
        settlement = e2e.get('settlement', {})
        result['e2e'] = {k: round(v, 1) if isinstance(v, float) else v for k, v in settlement.items()
                         if isinstance(v, (int, float)) and 'second' in k}
        result['e2e']['total_seconds'] = round(e2e['seconds'], 1)
    return result


if __name__ == '__main__':
    print(json.dumps(timings(*sys.argv[1:3]), indent=2))
