#!/usr/bin/env python3
"""Measure four SHA-256 blocks over shared bytes; localhost synthetic gate only."""
import hashlib
import argparse
import json
import os
import pathlib
import re
import socket
import subprocess
import time
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
UPSTREAM = ROOT / ".deps/co-snarks"
BIN = UPSTREAM / "target/release/co-noir"
NARGO = ROOT / ".deps/noir-1.0.0-rc.2/nargo"
CIRCUIT = ROOT / "circuits/row_hash/target/darkcall_row_hash.json"
PIN = "467a2abbad940d40ed78f38d424cc21920988976"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    flavor = parser.add_mutually_exclusive_group()
    flavor.add_argument("--native", action="store_true", help="use separately built -C target-cpu=native coNoir")
    flavor.add_argument("--aes-native", action="store_true", help="use -C target-cpu=native --cfg aes_armv8 coNoir")
    parser.add_argument("--input", type=pathlib.Path, help="use an existing ignored local synthetic Prover.toml")
    parser.add_argument("--circuit", choices=["row_hash", "poseidon_row"], default="row_hash")
    parser.add_argument("--expected-digest", type=pathlib.Path, help="Poseidon2 reference JSON generated from the exact same row")
    args = parser.parse_args()
    binary = UPSTREAM / "target-aes-native/release/co-noir" if args.aes_native else UPSTREAM / "target-native/release/co-noir" if args.native else BIN
    circuit = ROOT / f"circuits/{args.circuit}/target/darkcall_{args.circuit}.json"
    actual = subprocess.check_output(["git", "-C", str(UPSTREAM), "rev-parse", "HEAD"], text=True).strip()
    if actual != PIN or not binary.is_file():
        raise RuntimeError("Build the pinned coNoir first")
    if "nargo version = 1.0.0-rc.2\n" not in subprocess.check_output([str(NARGO), "--version"], text=True):
        raise RuntimeError("Wrong Noir binary")
    subprocess.run([str(NARGO), "compile", "--program-dir", str(ROOT / "circuits" / args.circuit)], check=True)
    run = ROOT / ".local" / ("mpc-" + ("row-hash" if args.circuit == "row_hash" else "poseidon-row") + "-" + time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()))
    run.mkdir(exist_ok=False)
    # The synthetic plaintext is local to this client/fixture generator. Never
    # record it in the public evidence. MPC parties receive replicated shares.
    if args.input:
        source = args.input.resolve()
        if not source.is_relative_to(ROOT / ".local") or source.stat().st_size > 16384:
            raise RuntimeError("Expected a small ignored local synthetic input")
        inputs = tomllib.loads(source.read_text())
        values = inputs.get("input")
        if set(inputs) != {"input"} or not isinstance(values, list) or len(values) != 224 or any(type(x) is not int or not 0 <= x < 256 for x in values):
            raise RuntimeError("Expected exactly 224 private bytes")
        message = bytes(values)
    else:
        message = os.urandom(224)
    (run / "Prover.toml").write_text("input = " + json.dumps(list(message)) + "\n")
    digest = hashlib.sha256(message).digest()
    expected = [int.from_bytes(digest[i:i + 4], "big") for i in range(0, 32, 4)]
    if args.circuit == "poseidon_row":
        if not args.expected_digest or not args.input:
            raise RuntimeError("Poseidon2 comparison requires an existing exact row and reference digest")
        expected = json.loads(args.expected_digest.read_text())["digest_words"]
        if not isinstance(expected, list) or len(expected) != 8 or any(type(x) is not int or not 0 <= x < 2**32 for x in expected):
            raise RuntimeError("Expected eight canonical digest words")
    elif args.expected_digest:
        raise RuntimeError("SHA-256 derives its reference from Python, without an external digest")
    summary = {
        "scope": "shared hash feasibility gate, not a complete native settlement proof",
        "hash": "sha256" if args.circuit == "row_hash" else "existing-bn254-poseidon2-eight-packed-elements",
        "source_pin": PIN, "independent_hosts": False, "zk_enabled": True,
        "private_bytes": 224, "compression_blocks": 4 if args.circuit == "row_hash" else None, "stages": [],
        "network_bytes_measured": False,
        "binary_flavor": "target-cpu=native; aes_armv8" if args.aes_native else "target-cpu=native" if args.native else "portable",
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    }
    env = {**os.environ, "RUST_LOG": "info", "RAYON_NUM_THREADS": "2"}

    def record(name, commands, timeout=600):
        print(name, "started", flush=True)
        stage = {"name": name}
        start = time.monotonic()
        children, handles, logs = [], [], []
        try:
            for i, command in enumerate(commands):
                log = run / f"{name}-{i}.log"
                logs.append(log)
                handle = log.open("wb")
                handles.append(handle)
                # macOS time reports the child maximum resident set in bytes.
                children.append(subprocess.Popen(
                    ["/usr/bin/time", "-l", str(binary), *map(str, command)],
                    cwd=run, env=env, stdout=handle, stderr=subprocess.STDOUT,
                    start_new_session=True,
                ))
            deadline = start + timeout
            while any(child.poll() is None for child in children):
                failed = [child.returncode for child in children if child.poll() not in (None, 0)]
                if failed:
                    raise RuntimeError(f"{name} failed: {failed}; inspect ignored logs")
                if time.monotonic() > deadline:
                    raise TimeoutError(name + " exceeded bounded timeout")
                time.sleep(0.2)
            if any(child.wait() for child in children):
                raise RuntimeError(name + " failed; inspect ignored logs")
            stage["passed"] = True
            print(name, "passed", flush=True)
        except BaseException as error:
            stage.update(passed=False, error=str(error))
            raise
        finally:
            for child in children:
                if child.poll() is None:
                    # Kill the process group, including the time-wrapped child.
                    import signal
                    os.killpg(child.pid, signal.SIGTERM)
            for child in children:
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
            for handle in handles:
                handle.close()
            stage["seconds"] = round(time.monotonic() - start, 3)
            stage["peak_rss_bytes_per_process"] = []
            for log in logs:
                matches = re.findall(r"(\d+)\s+maximum resident set size", log.read_text(errors="replace"))
                stage["peak_rss_bytes_per_process"].append(int(matches[-1]) if matches else None)
            summary["stages"].append(stage)
            (run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

    record("split", [["split-input", "--circuit", circuit, "--input", run / "Prover.toml", "--protocol", "REP3", "--out-dir", run]], 120)
    sockets, ports = [], []
    try:
        for _ in range(3):
            sock = socket.socket()
            sock.bind(("127.0.0.1", 0))
            sockets.append(sock)
            ports.append(sock.getsockname()[1])
        data = UPSTREAM / "co-noir/co-noir/examples/data"
        for party in range(3):
            text = f'[network]\nmy_id = {party}\nbind_addr = "127.0.0.1:{ports[party]}"\nmax_frame_length = 469762056\n'
            for peer, port in enumerate(ports):
                text += f'\n[[network.parties]]\nid = {peer}\ndns_name = "127.0.0.1:{port}"\n'
            text += '\n[network.tls]\nkey = ' + json.dumps(str(data / f"key{party}.der"))
            text += '\ncerts = ' + json.dumps([str(data / f"cert{i}.der") for i in range(3)]) + '\n'
            (run / f"party{party}.toml").write_text(text)
    finally:
        for sock in sockets:
            sock.close()
    record("witness", [[
        "generate-witness", "--circuit", circuit, "--protocol", "REP3",
        "--input", run / f"Prover.toml.{i}.shared", "--config", run / f"party{i}.toml",
        "--out", run / f"witness.{i}.shared",
    ] for i in range(3)], 1800)
    crs = UPSTREAM / "co-noir/co-noir-common/src/crs"
    record("vk", [["create-vk", "--circuit", circuit, "--crs", crs / "bn254_g1.dat", "--hasher", "poseidon2", "--vk", run / "vk"]], 600)
    record("proof", [[
        "build-and-generate-proof", "--circuit", circuit, "--protocol", "REP3",
        "--witness", run / f"witness.{i}.shared", "--config", run / f"party{i}.toml",
        "--crs", crs / "bn254_g1.dat", "--hasher", "poseidon2", "--vk", run / "vk",
        "--zk", "--out", run / f"proof.{i}", "--public-input", run / f"public-input{i}",
    ] for i in range(3)], 1800)
    command = ["verify", "--proof", run / "proof.0", "--vk", run / "vk", "--crs", crs / "bn254_g2.dat", "--hasher", "poseidon2", "--has-zk"]
    record("verify", [[*command, "--public-input", run / "public-input0"]], 300)
    output = (run / "public-input0").read_bytes()
    fields = [int.from_bytes(output[i:i + 32], "big") for i in range(0, len(output), 32)]
    if fields != expected:
        raise RuntimeError("MPC hash differs from its reference")
    for i in [1, 2]:
        if (run / f"public-input{i}").read_bytes() != output:
            raise RuntimeError("MPC parties disagree on the hash result")
    altered = bytearray(output)
    altered[:32] = (fields[0] ^ 1).to_bytes(32, "big")
    (run / "altered-digest").write_bytes(altered)
    with (run / "reject-digest.log").open("wb") as log:
        rejected = subprocess.run([str(binary), *map(str, [*command, "--public-input", run / "altered-digest"])], stdout=log, stderr=subprocess.STDOUT, timeout=300, env=env)
    if rejected.returncode == 0 or "Proof verification failed" not in (run / "reject-digest.log").read_text():
        raise RuntimeError("Digest substitution was not cryptographically rejected")
    summary.update(passed=True, reference_digest_matches=True, python_digest_matches=args.circuit == "row_hash", altered_digest_rejected=True, proof_bytes=(run / "proof.0").stat().st_size)
    (run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("Shared hash matches its reference; altered digest rejected. Evidence:", run, flush=True)


if __name__ == "__main__":
    main()
