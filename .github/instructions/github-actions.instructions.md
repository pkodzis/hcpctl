---
name: 'GitHub Actions CI/CD'
description: 'CI/CD workflow conventions for hcpctl release pipeline'
applyTo: '.github/workflows/**/*.yml'
---

# CI/CD Conventions for hcpctl

## Release Pipeline

- Uses **release-please** for automated versioning and changelog
- Cross-compilation for 6 targets: linux (amd64, amd64-musl, arm64), macOS (amd64, arm64), Windows (amd64)
- Binary name: `hcpctl`
- Uses `cross` for ARM64 builds
- Release profile: `strip = true`, `lto = true`, `codegen-units = 1`

## Workflow Structure

- `release.yml` — main release workflow triggered by push to `main`
- Supports `workflow_dispatch` for manual builds with version input
- release-please job → build matrix → upload artifacts

## Conventions

- Paths-ignore: `scripts/**`, `*.md`, `docs/**`
- Uses `dtolnay/rust-toolchain@stable` for Rust installation
- Always include `CARGO_TERM_COLOR: always` env var
