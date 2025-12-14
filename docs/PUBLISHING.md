# Publishing Checklist

## 1) Repo metadata
- Update `Cargo.toml`:
  - `repository = "..."`
  - `homepage = "..."`
  - `authors = [...]`
  - `keywords = [...]`
  - `categories = [...]`
- Update `README.md` with project links.

## 2) Licenses
This project is dual-licensed: **MIT OR Apache-2.0**.
Ensure that matches your intended distribution.

## 3) Keys & production hardening
- Set `UBL_API_KEY`
- Run behind TLS / reverse proxy
- Store signing keys securely (HSM or offline)
- Enable metrics / tracing export if needed

## 4) Release steps
- `cargo fmt && cargo clippy && cargo test`
- Tag a release: `git tag v2.1.0 && git push --tags`
- Create GitHub Release with `ubl_core_publication_final.zip` artifacts

## 5) Crates.io (optional)
- Ensure `name` is available
- `cargo publish`
