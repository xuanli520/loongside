# Linux Musl Release Contract Tasks

## Objective

Write, review, and commit the approved design for Linux GNU plus musl release
artifacts with libc-aware installer selection.

## Checklist

- [x] Inspect current installer, release helper, and release workflow behavior.
- [x] Confirm the Debian 12 failure mode and current public release contract.
- [x] Align the spec location and format with existing `docs/plans` documents.
- [x] Write `docs/plans/2026-03-20-linux-musl-release-contract-design.md`.
- [x] Perform a local review pass for contract gaps and scope drift.
- [ ] Commit the spec and task tracker updates.
- [ ] Ask for user review before writing the implementation plan.

## Progress Notes

- 2026-03-20: Confirmed the current Linux release contract is GNU-only in
  `scripts/release_artifact_lib.sh`, `scripts/install.sh`, and
  `.github/workflows/release.yml`.
- 2026-03-20: Confirmed the Bash installer is the Linux path; `install.ps1`
  remains Windows-only, so the first musl slice stays in the Bash/shared helper
  contract.
- 2026-03-20: Confirmed the release workflow already enforces a Linux ARM64
  glibc floor through `scripts/check_glibc_floor.sh`, which can be extended for
  explicit GNU floor metadata instead of inventing a second mechanism.
- 2026-03-20: Wrote the design doc in `docs/plans` and tightened the contract
  around explicit GNU override behavior, glibc detection order, and shared
  helper ownership.

## Review / Results

- 2026-03-20: Local review completed. The main gap was explicit override safety:
  the spec now requires the installer to fail early when `gnu` is forced on a
  host that does not meet the declared GNU glibc floor.
