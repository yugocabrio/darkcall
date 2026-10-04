# DarkCall linked SHA-256 extension

Upstream package: halo2_gadgets 0.5.0, registry source commit
`d751768afe0d2105b349dd93f73fde7f2eade088` in zcash/halo2.
The registry source files and normalized Cargo manifest are retained.
LICENSE-MIT and LICENSE-APACHE were fetched from that exact upstream commit.
The package remains under its original MIT OR Apache-2.0 license.
`UPSTREAM-FILES.sha256` records every retained original registry file. Only four
listed SHA source files differ; the added license/provenance files are explicit.

Changes are limited to SHA-256 Table16 and its compression assignment helpers:
unknown-capable initialization, exposure of constrained digest cells, and new
`initialize_linked`, `compress_linked`, `digest_linked` methods. The existing
instruction trait's behavior is preserved. Callers must provide standard padding
and constrained modulo-2^32 feed-forward. The original Table16 trait API alone
does not establish these application-level hash links.

The linked methods impose equality constraints between caller input/state cells
and their internal decompositions. Reading an AssignedCell's value to assign a
new witness is insufficient without these copy constraints. This extension is
experimental and has not been audited. Application tests and measured gates are
documented under docs/ and evidence/ in the parent repository.
