# Publishing Checklist

## Pre-Release

- [ ] Update `Cargo.toml` metadata (repository, homepage, authors, keywords)
- [ ] Verify license files (MIT OR Apache-2.0)
- [ ] Run `cargo fmt && cargo clippy && cargo test`
- [ ] Review and update README.md

## Production Configuration

- [ ] Set `UBL_API_KEY` environment variable
- [ ] Configure Ed25519 signing keys (optional but recommended)
- [ ] Deploy behind TLS/reverse proxy
- [ ] Store signing keys securely (HSM or offline storage)
- [ ] Enable structured logging/tracing export if needed

## Release

- [ ] Tag release: `git tag v2.1.0 && git push --tags`
- [ ] Create GitHub Release with release notes
- [ ] Attach build artifacts if needed

## Optional: Crates.io

- [ ] Verify package name availability
- [ ] Run `cargo publish --dry-run`
- [ ] Publish: `cargo publish`
