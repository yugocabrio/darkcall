#!/usr/bin/env python3
"""Synthetic, single-host MPC computation gate. Not a deployment privacy proof.

Each bidder file contains only one private bid. Shares, TLS test keys and witness
files stay under ignored .local/. Public summaries contain no private witness.
The circuit does not yet authenticate accepted inputs or native settlement.
"""

import json
import os
import pathlib
import socket
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / ".deps/co-snarks"
BIN = UPSTREAM / "target/release/co-noir"
CIRCUIT = ROOT / "circuits/auction4/target/darkcall_auction4.json"
NARGO = ROOT / ".deps/noir-1.0.0-rc.2/nargo"
PIN = "467a2abbad940d40ed78f38d424cc21920988976"


def main():
    actual_pin = subprocess.check_output(
        ["git", "-C", str(UPSTREAM), "rev-parse", "HEAD"], text=True
    ).strip()
    if actual_pin != PIN or not BIN.exists():
        raise RuntimeError("Build the pinned coNoir before running this experiment")
    version = subprocess.check_output([str(NARGO), "--version"], text=True)
    if "nargo version = 1.0.0-rc.2\n" not in version:
        raise RuntimeError("Wrong Noir version")
    subprocess.run([str(NARGO), "compile", "--program-dir", str(ROOT / "circuits/auction4")], check=True)
    run = ROOT / ".local" / ("mpc-auction4-" + time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()))
    run.mkdir(exist_ok=False)
    summary = {
        "source_pin": PIN,
        "scope": "synthetic calculation gate",
        "independent_hosts": False,
        "input_commitments": False,
        "native_settlement": False,
        "zk_enabled": True,
        "stages": [],
    }
    environment = {**os.environ, "RUST_LOG": "info", "RAYON_NUM_THREADS": "2"}

    def record(name, commands, timeout):
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
                    [str(BIN), *map(str, command)],
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

    bids = [90, 70, 60, 50]
    for i, bid in enumerate(bids):
        (run / ("bidder" + str(i) + ".toml")).write_text("bid" + str(i) + ' = "' + str(bid) + '"\n')
    ids = [[0] * 31 + [i] for i in range(4)]
    (run / "public.toml").write_text(
        "ids = " + json.dumps(ids) + "\n"
        + "active = [true, true, true, true]\nunits = 3\n"
        + 'reserve = "20"\nbudget = "100"\nprocessing_fee = "15000"\n'
    )
    for name in [*(("bidder" + str(i)) for i in range(4)), "public"]:
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
    crs = UPSTREAM / "co-noir/co-noir-common/src/crs"
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
    if fields[-2:] != [50, 3]:
        raise RuntimeError("Public result differs from independent fixture: " + str(fields[-2:]))
    summary.update(result={"price": 50, "sold": 3}, proof_bytes=(run / "proof.0").stat().st_size)
    for i in [1, 2]:
        if (run / ("public-input" + str(i))).read_bytes() != outputs:
            raise RuntimeError("MPC parties disagree on public output")
    # Alter the public result while retaining the original proof and key.
    bad = bytearray(outputs)
    bad[-64:-32] = (49).to_bytes(32, "big")
    (run / "tampered-public-input").write_bytes(bad)
    bad_command = [
        str(BIN), "verify", "--proof", str(run / "proof.0"),
        "--public-input", str(run / "tampered-public-input"), "--vk", str(run / "vk"),
        "--crs", str(crs / "bn254_g2.dat"), "--hasher", "poseidon2", "--has-zk",
    ]
    with (run / "tampered-verifier.log").open("wb") as log:
        result = subprocess.run(bad_command, stdout=log, stderr=subprocess.STDOUT, timeout=180, env=environment)
    if result.returncode == 0 or "Proof verification failed" not in (run / "tampered-verifier.log").read_text():
        raise RuntimeError("Verifier did not cryptographically reject changed clearing price")
    summary.update(tampered_price_rejected=True, external_barretenberg_verified=False)
    (run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("Verified collaborative ZK calculation and rejected changed price.", flush=True)
    print("Evidence directory:", run, flush=True)


if __name__ == "__main__":
    main()
