#!/usr/bin/env python3
"""Verify an existing synthetic coNoir run using the pinned C++ Barretenberg."""
import hashlib
import argparse
import json
import pathlib
import subprocess
import time
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / ".deps/barretenberg-src"
PIN = "87aa5283d5b0cab6632ed49fdd25f6548d2ba91f"
BB = SOURCE / "barretenberg/cpp/build-darkcall/bin/bb"
CRS = ROOT / ".deps/co-snarks/co-noir/co-noir-common/src/crs"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=pathlib.Path)
    parser.add_argument("--circuit", choices=["auction4", "row_hash", "settlement4", "settlement_poseidon4", "delivery4", "delivery_private4", "delivery_flags4", "poseidon_row"], default="auction4")
    parser.add_argument("--output-label", default="external-bb")
    args = parser.parse_args()
    if not re.fullmatch(r'[a-z0-9-]{1,80}', args.output_label):
        raise RuntimeError('Invalid verifier output label')
    circuit = ROOT / f"circuits/{args.circuit}/target/darkcall_{args.circuit}.json"
    tampering = {
        "auction4": [("price", "tampered-price"), ("accepted-root", "tampered-accepted-root")],
        "row_hash": [("digest", "altered-digest")],
        "poseidon_row": [("digest", "altered-digest")],
        "settlement4": [("price", "tampered-price"), ("plan", "tampered-plan"), ("row", "tampered-row")],
        "settlement_poseidon4": [("price", "tampered-price"), ("plan", "tampered-plan"), ("row", "tampered-row")],
        "delivery4": [("price", "tampered-price"), ("root", "tampered-root"), ("cipher", "tampered-cipher")],
        "delivery_flags4": [("price", "tampered-price"), ("root", "tampered-root"), ("cipher", "tampered-cipher"), ("winner", "tampered-winner")],
        "delivery_private4": [("price", "tampered-price"), ("root", "tampered-root"), ("cipher", "tampered-cipher")],
    }[args.circuit]
    run = args.run.resolve()
    if not run.is_relative_to(ROOT / ".local"):
        raise RuntimeError("This experimental verifier expects an ignored local fixture run")
    pin = subprocess.check_output(["git", "-C", str(SOURCE), "rev-parse", "HEAD"], text=True).strip()
    if pin != PIN or not BB.is_file():
        raise RuntimeError("Build the pinned C++ verifier first")
    actual_patch = subprocess.check_output(["git", "-C", str(SOURCE), "diff", "--"], text=True)
    expected_patch = (ROOT / "patches/barretenberg-apple-clang15.patch").read_text() + (ROOT / "patches/barretenberg-macos-ipc.patch").read_text()
    if actual_patch != expected_patch:
        raise RuntimeError("Unexpected source changes in the verifier checkout")
    out = run / args.output_label
    out.mkdir(exist_ok=False)
    summary = {"barretenberg_source": PIN, "circuit": args.circuit, "compatibility_patches": ["Apple Clang 15 deduction guide", "weak-link unavailable macOS IPC symbols; IPC explicitly throws when unavailable"], "zk": True, "passed": False}

    def call(name, command, expected=0):
        with (out / (name + ".log")).open("wb") as log:
            result = subprocess.run([str(BB), *map(str, command)], stdout=log, stderr=subprocess.STDOUT, timeout=180)
        if (expected == 0 and result.returncode != 0) or (expected != 0 and result.returncode == 0):
            raise RuntimeError(name + " unexpected verifier exit " + str(result.returncode))
        if expected != 0 and "Proof verification failed" not in (out / (name + ".log")).read_text():
            raise RuntimeError(name + " did not report cryptographic proof rejection")
        return result.returncode

    start = time.monotonic()
    try:
        call("trusted-vk", ["write_vk", "--scheme", "ultra_honk", "--verifier_target", "noir-recursive", "-c", CRS, "-b", circuit, "-o", out])
        vk = (out / "vk").read_bytes()
        if vk != (run / "vk").read_bytes():
            raise RuntimeError("C++ verification key differs from the MPC key")
        common = ["verify", "--scheme", "ultra_honk", "--verifier_target", "noir-recursive", "-c", CRS, "-p", run / "proof.0", "-k", out / "vk"]
        call("valid-proof", [*common, "-i", run / "public-input0"])
        for name, filename in tampering:
            call("reject-" + name, [*common, "-i", run / filename], expected=1)
        summary.update(passed=True, matching_key=True, cryptographic_rejections=[name for name, _ in tampering], circuit_sha256=hashlib.sha256(circuit.read_bytes()).hexdigest())
    except BaseException as error:
        summary["error"] = str(error)
        raise
    finally:
        summary["seconds"] = round(time.monotonic() - start, 3)
        (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
