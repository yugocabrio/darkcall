#!/usr/bin/env python3
"""Synthetic, single-host MPC computation gate. Not a deployment privacy proof.

Each client proves its own salted bid and all three delivered share pairs.
Nodes verify the proof and their pair before merging inputs. The MPC circuit
checks the same accepted commitments and root. Shares, fixture TLS keys and
witness files stay under ignored .local/. Funding and native settlement remain
outside this experiment.
"""

import argparse
import json
import os
import pathlib
import socket
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / ".deps/co-snarks"
BIN = UPSTREAM / "target-aes-native/release/co-noir"
PLAIN = UPSTREAM / "target/release/plaindriver"
INPUT_TOOL = ROOT / "target/release/darkcall-input-tool"
CIRCUIT = ROOT / "circuits/auction4/target/darkcall_auction4.json"
CLIENT_CIRCUIT = ROOT / "circuits/bid_input/target/darkcall_bid_input.json"
NARGO = ROOT / ".deps/noir-1.0.0-rc.2/nargo"
PIN = "467a2abbad940d40ed78f38d424cc21920988976"


def main():
    os.umask(0o077)  # Includes coNoir-created secret shares/witnesses.
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bids',type=int,nargs=4,default=[90,70,60,50],help='Synthetic client fixture only; never real bid arguments')
    parser.add_argument('--reserve',type=int,choices=(0,20),default=20,help='Synthetic context; zero enables zero-price fixtures')
    parser.add_argument('--result-file',type=pathlib.Path)
    args=parser.parse_args()
    if any(b<0 or b>100 for b in args.bids): raise RuntimeError('Fixture bids outside budget')
    if args.result_file is not None and not args.result_file.resolve().is_relative_to(ROOT / '.local'): raise RuntimeError('Result file must remain local')
    actual_pin = subprocess.check_output(
        ["git", "-C", str(UPSTREAM), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual_pin != PIN or not all(path.exists() for path in [BIN, PLAIN, INPUT_TOOL]):
        raise RuntimeError("Build the pinned coNoir before running this experiment")
    version = subprocess.check_output([str(NARGO), "--version"], text=True)
    if "nargo version = 1.0.0-rc.2\n" not in version:
        raise RuntimeError("Wrong Noir version")
    subprocess.run([str(NARGO), "compile", "--program-dir", str(ROOT / "circuits/auction4")], check=True)
    subprocess.run([str(NARGO), "compile", "--program-dir", str(ROOT / "circuits/bid_input")], check=True)
    run = ROOT / ".local" / ("mpc-auction4-" + time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()))
    run.mkdir(mode=0o700, exist_ok=False)
    summary = {
        "source_pin": PIN,
        "scope": "synthetic committed-input calculation gate",
        "independent_hosts": False,
        "input_commitments": True,
        "funding_authenticated": False,
        "native_settlement": False,
        "zk_enabled": True,
        "stages": [],
    }
    environment = {**os.environ, "RUST_LOG": "info", "RAYON_NUM_THREADS": "2"}

    def record(name, commands, timeout, binary=BIN):
        print(name, "started", flush=True)
        start = time.monotonic()
        children = []
        handles = []
        stage = {"name": name}
        try:
            for i, command in enumerate(commands):
                handle = (run / (name + "-" + str(i) + ".log")).open("wb")
                handles.append(handle)
                children.append(subprocess.Popen(
                    [str(binary), *map(str, command)],
                    stdout=handle, stderr=subprocess.STDOUT, env=environment,
                    cwd=run, start_new_session=True,
                ))
            expires = time.monotonic() + timeout
            while any(child.poll() is None for child in children):
                failed = [child.returncode for child in children if child.poll() not in (None, 0)]
                if failed:
                    raise RuntimeError(name + " child failed: " + str(failed))
                if time.monotonic() >= expires:
                    raise TimeoutError(name + " exceeded bounded timeout")
                time.sleep(0.2)
            codes = [child.wait() for child in children]
            if any(codes):
                raise RuntimeError(name + " failed: " + str(codes))
            stage["passed"] = True
            print(name, "passed", flush=True)
        except BaseException as error:
            stage.update(passed=False, error=str(error))
            raise
        finally:
            for child in children:
                if child.poll() is None:
                    child.terminate()
            for child in children:
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
            for handle in handles:
                handle.close()
            stage["seconds"] = round(time.monotonic() - start, 3)
            summary["stages"].append(stage)
            (run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    fixture = ["fixture", run, *args.bids] if args.reserve == 20 else ["fixture-reserve", run, args.reserve, *args.bids]
    record("client-fixture", [fixture], 60, INPUT_TOOL)
    crs = UPSTREAM / "co-noir/co-noir-common/src/crs"
    record("trusted-client-vk", [[
        "create-vk", "--circuit", CLIENT_CIRCUIT, "--crs", crs / "bn254_g1.dat",
        "--hasher", "poseidon2", "--vk", run / "client-vk",
    ]], 180)
    record("client-proofs", [[
        "--prover-crs", crs / "bn254_g1.dat", "--verifier-crs", crs / "bn254_g2.dat",
        "--input", run / f"client{i}/Prover.toml", "--circuit", CLIENT_CIRCUIT,
        "--hasher", "poseidon2", "--out-dir", run / f"client{i}", "--zk",
    ] for i in range(4)], 600, PLAIN)
    record("client-proof-verification", [[
        "verify", "--proof", run / f"client{i}/proof_plaindriver",
        "--public-input", run / f"client{i}/public_inputs_plaindriver",
        "--vk", run / "client-vk", "--crs", crs / "bn254_g2.dat",
        "--hasher", "poseidon2", "--has-zk",
    ] for i in range(4)], 180)
    record("delivered-share-checks", [[
        "check", run / "book.json", i, party, run / f"bidder{i}.toml.{party}.shared",
        run / f"client{i}/public_inputs_plaindriver",
    ] for i in range(4) for party in range(3)], 60, INPUT_TOOL)
    record("tamper-share", [[
        "tamper", run / "bidder0.toml.0.shared", run / "bad-share", 0,
    ]], 60, INPUT_TOOL)
    bad_share = subprocess.run([
        str(INPUT_TOOL), "check", str(run / "book.json"), "0", "0", str(run / "bad-share"),
        str(run / "client0/public_inputs_plaindriver"),
    ], capture_output=True, timeout=60)
    (run / "bad-share-check.log").write_bytes(bad_share.stdout + bad_share.stderr)
    if bad_share.returncode == 0 or b"delivered share pair does not match proof" not in bad_share.stderr:
        raise RuntimeError("Changed private share was not rejected before MPC")
    summary.update(delivered_share_tampering_rejected=True, client_proofs_verified=4, validated_share_pairs=12)
    for name in ["public"]:
        record("split-" + name, [[
            "split-input", "--circuit", CIRCUIT, "--input", run / (name + ".toml"),
            "--protocol", "REP3", "--out-dir", run,
        ]], 60)
    held = []
    ports = []
    for _ in range(3):
        sock = socket.socket()
        sock.bind(("127.0.0.1", 0))
        held.append(sock)
        ports.append(sock.getsockname()[1])
    try:
        for party in range(3):
            text = '[network]\nmy_id = ' + str(party)
            text += '\nbind_addr = "127.0.0.1:' + str(ports[party]) + '"\n'
            text += "max_frame_length = 469762056\n"
            for peer, port in enumerate(ports):
                text += '\n[[network.parties]]\nid = ' + str(peer)
                text += '\ndns_name = "127.0.0.1:' + str(port) + '"\n'
            data = UPSTREAM / "co-noir/co-noir/examples/data"
            text += '\n[network.tls]\nkey = ' + json.dumps(str(data / ("key" + str(party) + ".der")))
            text += "\ncerts = " + json.dumps([str(data / ("cert" + str(i) + ".der")) for i in range(3)]) + "\n"
            (run / ("party" + str(party) + ".toml")).write_text(text)
            inputs = []
            for name in [*(("bidder" + str(i)) for i in range(4)), "public"]:
                inputs.extend(["--inputs", run / (name + ".toml." + str(party) + ".shared")])
            record("merge-" + str(party), [[
                "merge-input-shares", "--circuit", CIRCUIT, "--protocol", "REP3",
                *inputs, "--out", run / ("input." + str(party) + ".shared"),
            ]], 60)
    finally:
        for sock in held:
            sock.close()

    record("witness", [[
        "generate-witness", "--circuit", CIRCUIT, "--protocol", "REP3",
        "--input", run / ("input." + str(i) + ".shared"),
        "--config", run / ("party" + str(i) + ".toml"),
        "--out", run / ("witness." + str(i) + ".shared"),
    ] for i in range(3)], 600)
    record("vk", [[
        "create-vk", "--circuit", CIRCUIT, "--crs", crs / "bn254_g1.dat",
        "--hasher", "poseidon2", "--vk", run / "vk",
    ]], 180)
    record("proof", [[
        "build-and-generate-proof", "--circuit", CIRCUIT, "--protocol", "REP3",
        "--witness", run / ("witness." + str(i) + ".shared"),
        "--config", run / ("party" + str(i) + ".toml"),
        "--crs", crs / "bn254_g1.dat", "--hasher", "poseidon2",
        "--vk", run / "vk", "--zk", "--out", run / ("proof." + str(i)),
        "--public-input", run / ("public-input" + str(i)),
    ] for i in range(3)], 600)
    record("verify", [[
        "verify", "--proof", run / "proof.0", "--public-input", run / "public-input0",
        "--vk", run / "vk", "--crs", crs / "bn254_g2.dat", "--hasher", "poseidon2", "--has-zk",
    ]], 180)
    outputs = (run / "public-input0").read_bytes()
    if len(outputs) % 32:
        raise RuntimeError("Unexpected public input encoding")
    fields = [int.from_bytes(outputs[i:i + 32], "big") for i in range(0, len(outputs), 32)]
    eligible=sorted((b for b in args.bids if b>=args.reserve),reverse=True)
    expected_price=eligible[3] if len(eligible)>3 else args.reserve
    expected_sold=min(3,len(eligible))
    if fields[-2:] != [expected_price, expected_sold]:
        raise RuntimeError("Public result differs from independent fixture: " + str(fields[-2:]))
    summary.update(result={"price": expected_price, "sold": expected_sold}, proof_bytes=(run / "proof.0").stat().st_size)
    for i in [1, 2]:
        if (run / ("public-input" + str(i))).read_bytes() != outputs:
            raise RuntimeError("MPC parties disagree on public output")
    # Bind both the result and the exact accepted-input root to the proof.
    for name, offset, value in [("price", -64, fields[-2]^1), ("accepted-root", -96, fields[-3] + 1)]:
        bad = bytearray(outputs)
        bad[offset:offset + 32] = value.to_bytes(32, "big")
        path = run / ("tampered-" + name)
        path.write_bytes(bad)
        command = [
            str(BIN), "verify", "--proof", str(run / "proof.0"),
            "--public-input", str(path), "--vk", str(run / "vk"),
            "--crs", str(crs / "bn254_g2.dat"), "--hasher", "poseidon2", "--has-zk",
        ]
        with (run / ("tampered-" + name + ".log")).open("wb") as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=180, env=environment)
        if result.returncode == 0 or "Proof verification failed" not in (run / ("tampered-" + name + ".log")).read_text():
            raise RuntimeError("Verifier did not cryptographically reject changed " + name)
    summary.update(tampered_price_rejected=True, tampered_root_rejected=True, external_barretenberg_verified=False)
    (run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("Verified client share proofs and committed MPC result; rejected share, price and root tampering.", flush=True)
    if args.result_file is not None:
        with args.result_file.open("x") as out: json.dump({"run":str(run)},out)
    print("Evidence directory:", run, flush=True)


if __name__ == "__main__":
    main()
