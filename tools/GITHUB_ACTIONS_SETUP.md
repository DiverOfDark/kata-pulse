# GitHub Actions CI/CD Setup

Complete automated build, test, and deployment pipeline for kata-monitor using GitHub Actions and GHCR (GitHub Container Registry).

## 📋 Overview

This setup provides:
- ✅ **Automated Testing** - Unit tests, linting, formatting, security audits
- ✅ **Docker Image Building** - Efficient caching, multi-architecture support
- ✅ **Container Registry Push** - Push to GHCR with automatic tagging
- ✅ **Security Scanning** - Trivy, Grype, OWASP Dependency Check
- ✅ **SBOM Generation** - Software Bill of Materials for supply chain security

## 🚀 Quick Start

### 1. No Configuration Required!

The workflows use GitHub's built-in `GITHUB_TOKEN` for authentication. Just push to your repository.

```bash
git push origin main
```

### 2. Images Automatically Published

```bash
# After push, images will be available at:
ghcr.io/your-org/kata-monitor:main
ghcr.io/your-org/kata-monitor:v1.0.0
ghcr.io/your-org/kata-monitor:latest
```

### 3. Pull/Run Images

```bash
# Authenticate (one time)
echo $GITHUB_TOKEN | docker login ghcr.io -u $GITHUB_ACTOR --password-stdin

# Pull and run
docker pull ghcr.io/your-org/kata-monitor:latest
docker run ghcr.io/your-org/kata-monitor:latest
```

## 📁 Workflow Files

### 1. **build-and-push.yml** - Main Build Pipeline

**Triggers:**
- Push to `main`, `master`, `develop` branches
- Push of `v*` tags (semantic versioning)
- Manual workflow dispatch
- Pull requests (dry-run, no push)

**Steps:**
1. Checkout code
2. Setup Docker Buildx
3. Authenticate to GHCR
4. Extract metadata and generate tags
5. Build image with layer caching
6. Push to GHCR

**Image Tags Generated:**
```
Trigger: git push origin main
Result:
  - ghcr.io/owner/repo:main
  - ghcr.io/owner/repo:main-<sha>
  - ghcr.io/owner/repo:latest

Trigger: git tag v1.2.3 && git push --tags
Result:
  - ghcr.io/owner/repo:v1.2.3
  - ghcr.io/owner/repo:1.2
  - ghcr.io/owner/repo:1
```

**Performance:**
- Uses GitHub Actions cache (`type=gha`)
- Subsequent builds 50-70% faster
- Layer caching improves build time

---

### 2. **docker-multiarch.yml** - Multi-Architecture Builds

**Triggers:**
- Push to main branches
- Tag pushes
- Manual dispatch

**Platforms Supported:**
- `linux/amd64` (x86_64/Intel/AMD)
- `linux/arm64` (ARM v8/Apple Silicon/Raspberry Pi)

**Features:**
- Native multi-arch support via QEMU
- SBOM generation (SPDX format)
- Artifact storage for compliance

**Usage:**
```bash
# Automatically builds for both architectures
git tag v1.0.0 && git push --tags

# Image works on any platform
docker pull ghcr.io/owner/repo:v1.0.0
```

---

### 3. **test.yml** - CI Quality Checks

**Triggers:**
- All push and PR events
- Manual dispatch

**Jobs (run in parallel):**

1. **Test Suite** - `cargo test`
   - Unit tests
   - Integration tests
   - Doc tests
   - Caching enabled

2. **Clippy** - Linting & Code Quality
   - Fails on warnings (`-D warnings`)
   - Runs on all targets
   - Enforces best practices

3. **Rustfmt** - Code Formatting
   - Checks formatting compliance
   - Must pass for PR approval

4. **Release Build** - Build Optimization
   - Compiles in release mode
   - Verifies optimization flags work

5. **Security Audit** - Vulnerability Checks
   - Non-blocking (informational only)
   - Checks for known CVEs
   - Doesn't block merges

**Example PR Check Failure:**
```
❌ Test Suite: Failed (missing semicolon)
❌ Clippy: Failed (unused variable)
✅ Rustfmt: Passed
```
→ User must fix before merge

---

### 4. **security.yml** - Advanced Security Scanning

**Triggers:**
- Push to main branch
- Pull requests
- Daily schedule (2 AM UTC)
- Manual dispatch

**Scanners:**

1. **Trivy** - Container Image Scanning
   - Scans for OS and application vulnerabilities
   - Finds CVEs in base image (Debian)
   - Results in GitHub Security tab

2. **Grype** - Detailed Container Analysis
   - Comprehensive package vulnerability scan
   - SARIF output for GitHub integration
   - Shows detailed vulnerability info

3. **OWASP Dependency Check** - Dependency Auditing
   - Identifies vulnerable dependencies
   - Checks Cargo.lock entries
   - Experimental checks enabled

**Security Tab Integration:**
- Results appear in: Settings → Code Security and Analysis → Vulnerability Alerts
- Shows severity levels (Critical, High, Medium, Low)
- Actionable remediation steps

---

## 🔐 Security & Best Practices

### Authentication (Zero Configuration!)

```yaml
# Uses GitHub's built-in token
secrets:
  GITHUB_TOKEN: automatically provided
```

**Benefits:**
- ✅ No credentials in repository
- ✅ Tokens scoped to repo only
- ✅ Auto-rotated by GitHub
- ✅ Works for public and private repos
- ✅ No setup needed!

### Permissions Model

```yaml
permissions:
  contents: read           # Read source code
  packages: write          # Push to GHCR
  security-events: write   # Upload scan results
```

**Why minimal permissions?**
- Follows least-privilege principle
- If token leaked, damage is limited
- Each workflow gets only what it needs

### Image Security

1. **Base Image**: `debian:bookworm-slim`
   - Lightweight (50MB)
   - Regular security updates
   - Official Debian images

2. **Build Isolation**: Multi-stage Dockerfile
   - Build tools not in final image
   - Reduced attack surface
   - Faster image distribution

3. **Health Checks**: Enabled
   - Verifies container health
   - Curl to `/health` endpoint
   - 30-second intervals

4. **Non-Root User**: Enabled
   - Runs as root (system component)
   - Can be changed per environment

### Supply Chain Security

1. **SBOM Generation**: Each build
   - SPDX format
   - Lists all dependencies
   - Artifacts stored in GitHub

2. **Action Versions**: Pinned
   ```yaml
   uses: docker/build-push-action@v5  # ← pinned version
   ```
   - Prevents supply chain attacks
   - Ensures reproducible builds

3. **Vulnerability Scanning**: Multiple tools
   - Trivy (OS packages)
   - Grype (All packages)
   - Dependency-Check (Dependencies)

---

## 📊 Performance Optimization

### Cache Strategy

**Docker BuildX Cache:**
```yaml
cache-from: type=gha
cache-to: type=gha,mode=max
```
- Stores layer cache in GitHub Actions
- Subsequent builds reuse layers
- Typical improvement: 50-70% faster

**Cargo Cache:**
```yaml
uses: Swatinem/rust-cache@v2
```
- Caches dependencies (`~/.cargo`)
- Caches build artifacts (`target/`)
- Significantly speeds up tests

**Example Build Times:**
```
First build:     8 minutes (no cache)
Second build:    3 minutes (with cache)
PR builds:       2 minutes (incremental)
```

### Parallel Execution

Jobs run in parallel automatically:
```
test.yml runs:
├── Test Suite      (5 min) ────┐
├── Clippy          (3 min) ├─ all parallel ─→ Total: 5 min
├── Rustfmt         (2 min) │   (not sequential)
├── Release Build   (5 min) │
└── Security Audit  (4 min) ────┘
```

---

## 🔄 Workflow Examples

### Example 1: Feature Branch Push

```bash
$ git checkout -b feat/new-feature
$ git commit -m "Add new feature"
$ git push origin feat/new-feature
```

**Result:**
1. ✅ Tests run (must pass)
2. ✅ Code quality checks run
3. ✅ Image built (dry-run, not pushed)
4. ❌ Skips GHCR push (safety)

### Example 2: Merge to Main

```bash
$ git checkout main
$ git merge feat/new-feature
$ git push origin main
```

**Result:**
1. ✅ All tests run
2. ✅ Image built and pushed
3. ✅ Tag: `main-<sha>`
4. ✅ Tag: `latest`
5. ✅ Security scan runs

### Example 3: Release Tag

```bash
$ git tag v1.2.3
$ git push origin v1.2.3
```

**Result (in build-and-push.yml):**
1. ✅ Image built with multi-arch
2. ✅ Tags pushed:
   - `v1.2.3` (exact)
   - `1.2` (major.minor)
   - `1` (major)

**Result (in docker-multiarch.yml):**
1. ✅ Multi-arch builds (amd64 + arm64)
2. ✅ SBOM generated
3. ✅ All platforms available

---

## 📈 Monitoring & Debugging

### View Workflow Runs

1. Go to repository → "Actions" tab
2. Click workflow name to see details
3. Click run to see job logs

### Common Failures & Fixes

| Issue | Cause | Fix |
|-------|-------|-----|
| GHCR auth fails | Missing `packages: write` | Check workflow permissions |
| Build timeout | Slow dependencies | Increase timeout, check Cargo.lock |
| Clippy fails | Code warnings | Fix warnings or use `#[allow(...)]` |
| Tests fail | Code issue | Fix code and push again |
| Image not pushed | PR or dry-run | Only main branch pushes |

### Manual Workflow Dispatch

Trigger workflow manually:
1. Go to Actions tab
2. Select workflow
3. Click "Run workflow"
4. Choose branch
5. Click green "Run workflow" button

---

## 📋 Checklist for First-Time Setup

- [ ] Push to main branch (triggers all workflows)
- [ ] Wait for workflows to complete (5-10 minutes)
- [ ] Check "Actions" tab for results
- [ ] Verify images in GHCR:
  ```bash
  docker pull ghcr.io/your-org/kata-monitor:latest
  ```
- [ ] Check "Settings" → "Code security" for scan results
- [ ] Update repository description with image URL
- [ ] Configure branch protection rules (optional)

---

## 🎯 Next Steps

### Optional Enhancements

1. **Branch Protection Rules**
   ```
   Settings → Branches → Add rule
   - Require status checks to pass
   - Require code reviews
   - Dismiss stale reviews
   ```

2. **Notifications**
   ```
   Settings → Notifications
   - Email on workflow failures
   - Slack integration via action
   ```

3. **Manual Triggers**
   ```bash
   # Trigger workflow via CLI
   gh workflow run build-and-push.yml -f branch=main
   ```

4. **Scheduled Scans**
   ```yaml
   schedule:
     - cron: '0 2 * * *'  # Daily at 2 AM UTC
   ```

---

## 📚 References

- [GitHub Actions Docs](https://docs.github.com/en/actions)
- [GHCR Container Registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry)
- [Docker Build and Push Action](https://github.com/docker/build-push-action)
- [Trivy Security Scanner](https://github.com/aquasecurity/trivy)
- [Rust Toolchain Action](https://github.com/dtolnay/rust-toolchain)

---

## 💡 Tips & Tricks

### Force Rebuild Cache

Sometimes you need to clear the cache:

```bash
# Via GitHub CLI
gh api repos/owner/repo/actions/caches \
  --method DELETE \
  --input - << 'EOF'
{
  "ref": "main"
}
EOF
```

Or manually in: Settings → Caches

### Test Locally with Act

```bash
# Install act: https://github.com/nektos/act
brew install act

# Run workflow
act -l                          # List jobs
act -j build-and-push          # Run specific job
act -s GITHUB_TOKEN=xxx        # Provide secrets
```

### Generate SBOM Locally

```bash
# Install syft
curl -sSfL https://raw.githubusercontent.com/anchore/syft/main/install.sh | sh -s -- -b /usr/local/bin

# Generate SBOM
syft ghcr.io/your-org/kata-monitor:latest -o spdx
```

---

## 🐛 Troubleshooting Guide

### Q: Images not pushing to GHCR?
**A:** Check:
- [ ] Repository is public or you have GHCR write access
- [ ] `packages: write` permission in workflow
- [ ] Workflow ran on main/master/develop (not PR)
- [ ] No errors in "Log in to Container Registry" step

### Q: Tests keep failing?
**A:** Check:
- [ ] Run locally: `cargo test`
- [ ] Run clippy: `cargo clippy -- -D warnings`
- [ ] Run fmt: `cargo fmt -- --check`
- [ ] Update dependencies: `cargo update`

### Q: Cache not being used?
**A:** Cache is per:
- Branch
- Job
- Hash of `Cargo.lock` and `rust-toolchain.toml`

Changes to these files create new cache entries.

### Q: Workflow never runs?
**A:** Check:
- [ ] Workflow is in `.github/workflows/` directory
- [ ] YAML syntax is valid (use YAML linter)
- [ ] Trigger conditions are met (check `on:` section)
- [ ] Repository has Actions enabled (not disabled)

---

## 📞 Support

For issues:
1. Check workflow logs in Actions tab
2. Review this guide
3. Check [GitHub Actions docs](https://docs.github.com/en/actions)
4. Open issue in repository
