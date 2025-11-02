_default:
    @just --list

# Run clippy fixes
[group('fix')]
fix-lint:
    cargo clippy --all-targets --all-features --fix --allow-staged

# Run formatting fixes
[group('fix')]
fix-fmt:
    cargo fmt

# Run all fixes
[group('fix')]
fix: fix-lint fix-fmt

# Run tests
[group('check')]
check-test:
    cargo test --all-features

# Run clippy checks
[group('check')]
check-lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Run formatting checks
[group('check')]
check-fmt:
    cargo fmt -- --check

# Run all checks
[group('check')]
check: check-test check-lint check-fmt

# Publish a new version. Usage: just publish patch|minor|major.
[group('publish')]
publish MODE:
    just _publish-check-mode "{{MODE}}"
    just check
    just _check-uncommitted-changes
    dist build
    cargo bump {{MODE}}
    git add --all
    git commit -m "Bump version v`just _get-version`"
    git tag "v`just _get-version`"
    git push
    git push --tags

_publish-check-mode MODE:
    @[[ "{{MODE}}" =~ ^(patch|minor|major)$ ]] || (echo "Error: MODE must be patch, minor, or major" && exit 1)

_check-uncommitted-changes:
    @test -z "$(git status -s)" || (echo "Error: There are uncommitted changes" && exit 1)

_get-version:
    @cargo metadata --format-version=1 --no-deps | jq -r '.packages[0].version'
