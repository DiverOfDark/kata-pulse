# CI/CD Workflows

This directory contains the unified CI/CD pipeline for kata-pulse.

## Overview

The `ci-cd.yml` workflow combines all quality checks, Docker builds, and security scanning into a single comprehensive pipeline that runs on push, pull requests, and scheduled times.

## Workflow Stages

### Stage 1: Quality Checks (Parallel Execution)

These jobs run in parallel and must all pass before Docker builds are triggered.

#### `test-suite`
- Runs unit tests: `cargo test --verbose`
- Runs doc tests: `cargo test --doc`
- Installs system dependencies (protobuf-compiler, pkg-config, libssl-dev)
- Uses Rust cache for faster builds

#### `clippy`
- Lint checks: `cargo clippy --all-targets --all-features -- -D warnings`
- Enforces clippy warnings as errors
- Catches potential bugs and style issues

#### `fmt`
- Code formatting check: `cargo fmt -- --check`
- Ensures consistent code style
- No auto-fix, fails if formatting is incorrect

#### `build-release`
- Optimized release build: `cargo build --release --verbose`
- Validates that release mode compiles without warnings
- Creates optimized binary

#### `security-audit`
- Runs `rustsec` dependency audit
- Checks for known security vulnerabilities in dependencies
- Continues on error (audit warnings don't block pipeline)

**Timing**: All 5 jobs run in parallel. Typical duration: 3-5 minutes depending on cache state.

### Stage 2: Docker Build & Push (Conditional)

Runs only if all Stage 1 quality checks pass.

#### `build-and-push-docker`
- **Trigger**: Only on push events or tags (not on pull requests)
- **Platforms**: Always builds multi-architecture (amd64, arm64)
- **QEMU**: Sets up QEMU for cross-compilation
- **Buildx**: Uses Docker Buildx for multi-platform builds
- **Registry**: Pushes to GitHub Container Registry (ghcr.io)
- **Tags**: Applied based on:
  - Branch (e.g., `main`, `develop`)
  - Semantic version tags (e.g., `v1.0.0` → `1.0.0`, `1.0`, `1`)
  - Commit SHA (e.g., `main-abc123def`)
  - `latest` tag for default branch
- **SBOM**: Generates Software Bill of Materials in SPDX format
- **Cache**: Uses GitHub Actions cache for layer caching

**Output**:
- Multi-arch Docker image pushed to `ghcr.io/kata-containers/kata-pulse:tag`
- SBOM artifact uploaded (available in Actions)

**Timing**: ~2-5 minutes depending on layer cache hits

### Stage 3: Security Scanning (Conditional)

Runs only on push events or scheduled triggers (not on pull requests). Scans the built Docker image for vulnerabilities.

#### `trivy-scan`
- Container vulnerability scanner from Aqua Security
- Scans for CVEs, misconfigurations, secrets
- Output format: SARIF (GitHub security format)
- Automatically uploads results to GitHub Security tab

#### `grype-scan`
- Package vulnerability scanner from Anchore
- Identifies vulnerable dependencies in image
- Output format: SARIF
- Results uploaded to GitHub Security tab

#### `dependency-check`
- OWASP Dependency Check for comprehensive CVE analysis
- Experimental and retired CVE detection enabled
- Output format: SARIF
- Results available in GitHub Security tab

**Timing**: ~3-8 minutes total (runs in parallel)

### Stage 4: CI Pipeline Complete (Summary)

Final job that runs regardless of other job outcomes (uses `if: always()`).

#### `ci-pipeline-complete`
- Collects results from all upstream jobs
- Displays pass/fail status for each stage
- Fails pipeline if any required check (test, clippy, fmt, build-release) failed
- Security audit failures don't block (continue-on-error: true)
- Security scanning failures don't block (independent of main pipeline)

**Purpose**: Provides clear summary of pipeline status and enforces quality gates.

## Execution Flow

```
┌─────────────────────────────────────────────────────────┐
│ Push / Pull Request / Scheduled Trigger                │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Stage 1: Quality Checks (PARALLEL)                      │
│                                                         │
│  test-suite ┐                                           │
│  clippy     ├─→ All must pass                           │
│  fmt        ├─→ [~3-5 min]                              │
│  build-release┤                                         │
│  security-audit┘ (non-blocking)                         │
└────────────────┬────────────────────────────────────────┘
                 │
        ┌────────▼────────┐
        │ All pass?       │
        ├─────────────────┤
        │ Yes  │  No      │
        │      │          │
        ▼      ▼          ▼
       ┌─┐   FAIL     (PR/non-push)
       │✓│
       └─┼────────────────────────────────────────────────┐
         │                                                 │
         ▼                                                 │
┌─────────────────────────────────────────────────────────┤
│ Stage 2: Docker Build (CONDITIONAL: push only)          │
│                                                         │
│  build-and-push-docker (multi-arch)                     │
│  ├─ QEMU setup                                          │
│  ├─ Buildx multi-platform build                         │
│  ├─ Push to ghcr.io                                     │
│  └─ Generate SBOM                                       │
│  [~2-5 min]                                             │
└────────────────┬────────────────────────────────────────┤
                 │                                         │
                 ▼                                         │
┌─────────────────────────────────────────────────────────┤
│ Stage 3: Security Scans (CONDITIONAL: push/schedule)   │
│                                                         │
│  trivy-scan      ┐                                       │
│  grype-scan      ├─ All run in parallel                 │
│  dependency-check┘ Non-blocking failures                │
│  [~3-8 min]                                             │
└────────────────┬────────────────────────────────────────┤
                 │                                         │
                 ▼                                         │
┌─────────────────────────────────────────────────────────┤
│ Stage 4: Summary (always runs)                          │
│                                                         │
│  ci-pipeline-complete                                   │
│  ├─ Report status                                       │
│  └─ Enforce quality gates                               │
│  [~10 sec]                                              │
└─────────────────────────────────────────────────────────┘
        │
        ▼
    ✅ PASS  or  ❌ FAIL
```

## Triggers

### On Push
- To branches: `main`, `master`, `develop`
- To tags: `v*` (semantic versions)
- Actions: Full pipeline including security scans

### On Pull Request
- To branches: `main`, `master`, `develop`
- Actions: Quality checks only (no Docker push, no security scans)

### Scheduled
- Daily at 2 AM UTC
- Runs full pipeline including security scans
- Good for catching dependency vulnerabilities

### Manual Trigger
- Available via workflow_dispatch
- Useful for testing or forced reruns

## Configuration

### Environment Variables

```yaml
CARGO_TERM_COLOR: always      # Colorized Cargo output
RUST_BACKTRACE: 1             # Show backtraces on panic
REGISTRY: ghcr.io             # Container registry
IMAGE_NAME: kata-containers/kata-pulse  # Image name
```

### Secrets Required

- `GITHUB_TOKEN`: Automatically provided by GitHub Actions
  - Used for: Docker registry authentication, security uploads

## Status Badges

Add to repository README:

```markdown
[![CI/CD Pipeline](https://github.com/kata-containers/kata-pulse/actions/workflows/ci-cd.yml/badge.svg)](https://github.com/kata-containers/kata-pulse/actions/workflows/ci-cd.yml)
```

## Troubleshooting

### Tests Failing

1. Check job logs: Actions tab → workflow run → test-suite job
2. Run locally: `cargo test --verbose`
3. Check dependencies: `cargo tree`

### Clippy Warnings

1. Check clippy job output
2. Run locally: `cargo clippy --all-targets --all-features -- -D warnings`
3. Clippy suggestions are usually safe to follow

### Formatting Issues

1. Check fmt job output (shows diff)
2. Run locally: `cargo fmt`
3. Commit the formatted code

### Docker Build Failing

1. Verify Dockerfile compiles: `docker build .`
2. Check system dependencies in Dockerfile
3. Ensure all build flags are correct

### Security Scan Alerts

1. Check GitHub Security tab for detailed reports
2. For CVEs in dependencies:
   - Update Cargo.toml versions
   - Or add policy exceptions if CVE is not applicable
3. For secrets detected:
   - Don't commit secrets
   - Rotate any exposed credentials

### GHCR Push Failing

1. Verify GitHub token is available
2. Check repository is public or token has `write:packages` permission
3. Verify image name is correct: `ghcr.io/kata-containers/kata-pulse`

## Performance Tips

1. **First run is slower**: Docker layer cache is empty, security scans run fully
2. **Subsequent runs are faster**: GitHub Actions cache speeds up builds
3. **Multi-arch build is slower**: Building for both amd64 and arm64 takes time
4. **Use draft PRs**: Only triggers on workflow_dispatch to save CI minutes

## Security Best Practices

1. **Never** commit secrets (passwords, tokens, keys)
2. **Always** keep dependencies updated
3. **Review** security scan results regularly
4. **Use** branch protection rules to require passing checks
5. **Monitor** for dependency vulnerability alerts

## Maintenance

- Update action versions periodically (check for newer versions)
- Review Trivy/Grype policies if too many false positives
- Adjust timeouts if jobs timeout consistently
- Monitor GitHub Actions minutes usage

## Future Improvements

- [ ] Add code coverage reporting (codecov)
- [ ] Add SAST scanning (CodeQL, SEMGREP)
- [ ] Add performance benchmarking
- [ ] Add deployment to staging environment on successful build
- [ ] Add automated release creation for semantic version tags
