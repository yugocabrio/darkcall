# Pinned dependencies and notices

DarkCall's own source uses MIT (`LICENSE`). Preserve upstream notices for copied
vendor source and external tools. The generated inventory records 431 packages
resolved by Cargo.lock on aarch64-apple-darwin, their declared license expressions,
registry checksums and exact git sources; no declaration is missing in this graph.
It contains repository paths, never local absolute paths or private artifacts.

```sh
python3 scripts/dependency_inventory.py --target aarch64-apple-darwin --output evidence/2026-10-05-dependency-inventory.json
```

The command is locked and offline. Other targets need their platform crates cached
before inventory. The first unfiltered offline metadata attempt encountered an
uncached Android package; selecting the actual build target resolved that without
changing the lock or downloading platform dependencies.

| Component | Pin | Declared license / local source |
| --- | --- | --- |
| Noir/Nargo | 1.0.0-rc.2 | MIT OR Apache-2.0; pinned source workspace |
| coNoir / coSNARK / mpc-core | 467a2abbad940d40ed78f38d424cc21920988976 | MIT OR Apache-2.0; upstream workspace and package manifests |
| C++ Barretenberg | 87aa5283d5b0cab6632ed49fdd25f6548d2ba91f | Apache-2.0; barretenberg/LICENSE |
| Orchard native fork | 42015f15733b95e179189628d6fa34d92b5b3b8b | MIT OR Apache-2.0 |
| RedDSA FROST fork | ed49e9ca0699a6450f6d4a9fe62ff168f5ea1ead | MIT OR Apache-2.0 |
| pczt | 0.8.0-rc.1 | MIT OR Apache-2.0 |
| zcash_primitives / protocol / backend | 0.29.0 / 0.10.0 / 0.24.0-rc.1 | MIT OR Apache-2.0 |
| halo2_proofs | 0.3.5 | MIT OR Apache-2.0 |
| halo2_gadgets copied vendor | 0.5.0 | MIT OR Apache-2.0; both upstream LICENSE files retained |
| pasta_curves | 0.5.2 | MIT OR Apache-2.0 |
| ark-bn254 | 0.6.0 | MIT OR Apache-2.0 |
| ed25519-dalek | 2.2.0 | BSD-3-Clause |
| Zebra node | 6.3.0 | MIT OR Apache-2.0; both official archive LICENSE files retained |
| Separate NU7 Zebra node | 7.0.0-rc.0 | MIT OR Apache-2.0; both official archive LICENSE files retained |

Source declarations are from the resolved package manifests. Compiler and node
workspace declarations are also confirmed in the pinned [Noir source](https://github.com/noir-lang/noir/blob/v1.0.0-rc.2/Cargo.toml)
and [Zebra source](https://github.com/ZcashFoundation/zebra/blob/v6.3.0/Cargo.toml).
coSNARK carries a GPL file for its coCircom compiler path; its coNoir and mpc-core
manifests inherit the MIT OR Apache-2.0 workspace declaration. DarkCall's current
graph does not build coCircom. External binaries have additional dependencies;
the 431-package export covers this workspace target, not those separate graphs
or every system package in the pinned Ubuntu runtime image.

The two macOS Barretenberg compatibility patches are preserved under patches/
and checked by the external verifier. The copied halo2_gadgets source retains
its license files; changes constrain source-cell bindings used by native proof
circuits. Details are in output-binding/non-native Poseidon documentation.
Licenses here describe source metadata; this is not an audit or legal approval
of distribution. Full external-tool notices and other target inventories belong
in the final release package. The human controls OSS publication.


The separate NU7 SDK workspace is pinned to librustzcash commit
`9c5705f56517c040c50f96d29f1ff15a9383f3e0`; it does not alter the root lock.
The refreshed selected-target NU7 inventory includes462 packages, no missing
license declarations, and lock SHA256
`3d6f69bc6ad7286a844b5c40d5053242a4e7191d7f4b518a734dbc4b2074d882`.
Authenticated admission reuses the existing off-chain claim/receipt/journal source
through the local `darkcall-wallet-protocol` package. It intentionally excludes
the old native transaction/backend SDK: putting both full SDK graphs together
conflicted on bip32 prerelease/stable versions. The new lock retains all327 prior
package identities/checksums and adds179 entries for those existing primitives.
The root lock only adds the local package; all existing external versions and
checksums remain unchanged. No signature domain, native binding circuit or
verifier changes. Both selected-target inventories are refreshed; these are
source-license declarations, not binary dependency inventories or legal approval.

```sh
python3 scripts/dependency_inventory.py --manifest tools/nu7-compat/Cargo.toml --output evidence/2026-10-05-nu7-dependency-inventory.json
```

The committed canonical public bundle contains only transaction/proofs/public
statements, commitments and public verification-key comparison bytes. It contains
no viewing/spending key, PCZT, witness, plaintext maximum bid, delivery pad or
blinder. Those verifier keys are compared to our pinned compiled circuit and
never treated as bundle-provided trust anchors.

The separate NU7 regtest uses the official
[Zebra7.0.0-rc.0 release](https://github.com/ZcashFoundation/zebra/releases/tag/v7.0.0-rc.0).
The ARM Linux archive is SHA256
`59adbb7d3e19c0fb7370eeca628787973ee347e421700a36e3b8dbde5c0b86c4`,
with `LICENSE-APACHE` and `LICENSE-MIT` retained in the verified archive.
Container image is
`ubuntu@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55`.
This separate node adds no Rust dependency or lock change; archive/source notices
are additional to the Cargo inventories. Full commands and resource limits are in
`evidence/2026-10-05-nu7-funded-dkg-transfer.json`.
