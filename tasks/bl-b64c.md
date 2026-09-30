+++
title = "Coverage to 100% and raise scripts/check-coverage.sh floor from 55 back to 100"
created = 1790735966
updated = 1790735966
root_commit = "a1cd4a50828f7df4e52044aa655d01e24c8b5ac9"
+++
The builder's first run of make check (bl-b8ad) measured 55.51% line coverage (push.rs, sync.rs, main.rs, jira/client.rs, auth/oauth*.rs are the bulk). The old pre-commit hook never ran coverage, so 100% was never enforced. bl-b8ad set COVERAGE_THRESHOLD's default to 55 as a ratchet floor; this ball writes the tests and restores 100.