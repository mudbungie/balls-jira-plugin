# balls-plugin-jira — agent guide

Task tracking is `bl` (balls): `bl prime --as <identity>`, `bl list`, claim,
edit in the claimed worktree, `bl close`. Never `--no-verify`.

## The gate

`make check` is the complete gate: `test → clippy -D warnings → line cap
(scripts/check-line-lengths.sh: Rust files under 300 lines) →
scripts/check-coverage.sh (cargo tarpaulin, 100% line coverage)`. The
pre-commit hook (`scripts/pre-commit`, seated in `.git/hooks` by
`scripts/install-hooks.sh`, once per clone) does not run it on this machine —
this laptop does not compile in a gate (ops bl-3166, `~/ops/remote-builds.md`
"Phase 2"), and `cargo tarpaulin` / `cargo llvm-cov` are shimmed here and
refuse to run. The hook is one line, `exec bl-gate "$@"` (userconf, on PATH).
bl-gate exports `BALLS_TOOLCHAIN` (`rustc -V`; `rust-toolchain.toml` pins
1.95.0 so the string is byte-identical on the builder), asks `bl-speculate
check` for a verified verdict on the staged tree, and otherwise has the
noodlezoo builder run `make check` on that tree and sign one
(`bl-remote-gate`): exit 0 is a pass, 1 means the builder failed the tree
(read `ssh builder cat /tank/build/out/<sha>/log`), 75 means no verdict —
nothing recorded, commit refused, never `cargo test` instead. To run tests
before committing, `bl-remote-run <target>` runs any make target on the
builder and streams its log.

**All tests must pass and coverage must not fall below the floor before
anything merges.** It does not matter who broke the test.
