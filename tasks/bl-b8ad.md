+++
title = "Collapse pre-commit gate to exec bl-gate; make check is the whole gate (ops bl-3166)"
created = 1790735914
updated = 1790735914
root_commit = "a1cd4a50828f7df4e52044aa655d01e24c8b5ac9"
+++
Phase 2 of ops bl-3166 / ~/ops/remote-builds.md 'Phase 2'. scripts/pre-commit becomes exec bl-gate; add rust-toolchain.toml (builder image has no default toolchain); AGENTS.md 'The gate'; README hook text. Prove: first commit passes via the noodlezoo builder.