<!-- SPDX-License-Identifier: Apache-2.0 -->

## What changed and why

<!-- Describe the change and the reason for it. Keep it to what a reviewer needs. -->

Closes #NNN

## Checklist

- [ ] Local gates pass: `cargo fmt --all --check`, `cargo clippy ... -D warnings`, `cargo nextest run`, `cargo test --doc`, `cargo doc` (with `RUSTDOCFLAGS=-D warnings`), and `cargo deny check`.
- [ ] `CHANGELOG.md` has an entry, if the change is user-visible.
- [ ] Docs are updated, if behavior changed.
- [ ] Every commit is signed.
- [ ] No AI or assistant attribution anywhere in the commits or this PR.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full contribution guide.
