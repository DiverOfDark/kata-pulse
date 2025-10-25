# CI/CD Quick Reference

Quick commands and checklists for working with the kata-pulse CI/CD pipeline.

## Pre-Push Checklist

Before pushing code, run these locally to catch issues early:

```bash
# 1. Format code
cargo fmt

# 2. Run linter
cargo clippy --all-targets --all-features -- -D warnings

# 3. Run all tests
cargo test --verbose

# 4. Build release (optional but recommended)
cargo build --release
```

If all pass, you're ready to push!

## Common Workflows

### Regular Feature Development

```bash
# Create feature branch
git checkout -b feat/my-feature

# Make changes
# ... edit files ...

# Run pre-push checks
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose

# Commit
git add .
git commit -m "Add my feature"

# Push
git push origin feat/my-feature

# Open PR on GitHub
# CI pipeline runs automatically (quality checks only on PR)
```

### Release Flow

```bash
# Create release branch
git checkout -b release/v1.2.0

# Update version in Cargo.toml
vim Cargo.toml
# Change version = "1.1.0" to version = "1.2.0"

# Run final checks
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose
cargo build --release

# Commit version bump
git add Cargo.toml
git commit -m "Release v1.2.0"

# Merge to main
git checkout main
git merge release/v1.2.0
git push origin main

# Tag the release
git tag v1.2.0
git push origin v1.2.0

# CI Pipeline runs:
# - Quality checks (parallel)
# - Docker build (multi-arch, amd64+arm64)
# - Push to ghcr.io as "v1.2.0", "1.2.0", "1.2", "1", "latest"
# - Security scans
```

### Hotfix Flow

```bash
# Create hotfix branch from main
git checkout -b hotfix/critical-bug main

# Make fix
# ... edit files ...

# Run checks
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --verbose

# Commit
git add .
git commit -m "Fix critical bug in X"

# Merge to main
git checkout main
git merge hotfix/critical-bug
git push origin main

# CI runs full pipeline
```

### Manual Workflow Trigger

```bash
# Via GitHub CLI
gh workflow run ci-cd.yml --ref main

# Via GitHub UI
# 1. Go to Actions tab
# 2. Click "CI/CD Pipeline"
# 3. Click "Run workflow" → "Run workflow"
```

## Monitoring Pipeline

### View Pipeline Status

```bash
# Via GitHub CLI
gh run list --workflow ci-cd.yml --limit 5

# Via GitHub UI
# 1. Go to Actions tab
# 2. Click latest run
# 3. View job status and logs
```

### Common Commands

```bash
# Get latest run status
gh run list --workflow ci-cd.yml --limit 1 --json status,conclusion

# View latest run logs
gh run view --log --workflow ci-cd.yml

# Cancel a running workflow
gh run cancel <RUN_ID>

# Rerun failed jobs
gh run rerun <RUN_ID>
```

## Troubleshooting Quick Fixes

### Tests Failing

```bash
# Run tests locally with output
cargo test -- --nocapture --test-threads=1

# Run specific test
cargo test test_name -- --nocapture

# Run with debugging
RUST_LOG=debug cargo test -- --nocapture
```

### Clippy Failing

```bash
# See what clippy complains about
cargo clippy --all-targets --all-features

# The fix is usually in the suggestion
# Example: "consider using X instead of Y"

# Auto-fix some issues (experimental)
cargo clippy --fix
```

### Formatting Failing

```bash
# See what needs formatting
cargo fmt -- --check

# Auto-format
cargo fmt

# Commit and push
git add .
git commit -m "Format code"
git push
```

### Build Failing

```bash
# Clean and rebuild
cargo clean
cargo build --release --verbose

# Check for missing dependencies
cargo tree

# Check for deprecated APIs
cargo fix --allow-dirty
```

## Pipeline Performance Tips

| Step | Typical Time | How to Optimize |
|------|------------|-----------------|
| test-suite | 60-90s | Split tests, use cache |
| clippy | 30-50s | Already fast |
| fmt | 10-20s | Run locally first |
| build-release | 90-120s | Use Cargo cache |
| Docker build | 120-300s | Layer cache, multi-arch slow |
| Trivy scan | 30-60s | Container size |
| Grype scan | 30-60s | Number of dependencies |
| Dependency-check | 60-120s | Full CycloneDX generation |

**Total typical time**: 5-10 minutes

## Docker Commands

### Build Locally

```bash
# Single platform (for testing)
docker build -t kata-pulse:local .

# Multi-platform (requires buildx, same as CI)
docker buildx build --platform linux/amd64,linux/arm64 -t kata-pulse:test .
```

### Test Image

```bash
docker run -it kata-pulse:local \
  -p 8090:8090 \
  -v /run/kata:/run/kata:ro \
  /target/release/kata-pulse --help
```

## Checking Security Scan Results

### GitHub Security Tab

```bash
# Via GitHub CLI
gh api repos/kata-containers/kata-pulse/security/advisories

# Via GitHub UI
1. Go to Security tab
2. Click "Code scanning alerts"
3. Review each alert with details
```

### Local CVE Scanning

```bash
# Install and run Trivy locally
trivy image kata-pulse:local

# Install and run Grype locally
grype kata-pulse:local

# Check dependencies with cargo-audit
cargo install cargo-audit
cargo audit
```

## Useful Links

- [GitHub Actions Documentation](https://docs.github.com/actions)
- [Rust Toolchain](https://github.com/dtolnay/rust-toolchain)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Clippy Lints](https://doc.rust-lang.org/clippy/)
- [Trivy Documentation](https://aquasecurity.github.io/trivy/)
- [Grype Documentation](https://github.com/anchore/grype)
- [Dependency-Check](https://owasp.org/www-project-dependency-check/)

## Emergency: Force Push (⚠️ Use Caution)

```bash
# Only if absolutely necessary and all tests pass locally
git push --force-with-lease origin feat/my-feature

# Note: Prefer rebasing and normal push when possible
git rebase main
git push origin feat/my-feature
```

## Debug: Run Step in SSH

If workflow is mysteriously failing, you can debug directly:

```bash
# Via GitHub CLI (requires GitHub CLI 2.17.0+)
gh run view RUN_ID --log
```

Or add this step to workflow for manual SSH access:

```yaml
- name: Setup tmate session
  uses: mxschmitt/action-tmate@v3
  if: failure()
```

Then in workflow logs you'll get an SSH URL to debug the runner interactively.
