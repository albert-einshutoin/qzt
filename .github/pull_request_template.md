## Why

<!-- Explain the product, security, compatibility, or maintenance problem. -->

## Before

<!-- Describe the observable behavior before this change. -->

## After

<!-- Describe the observable behavior after this change. -->

## Scope and decisions

<!-- Record important design choices and why this PR stays reviewably small. -->

## Verification

<!-- Replace or extend these commands with focused evidence for the change. -->

```sh
make check
```

- [ ] Focused tests failed for the expected reason before implementation.
- [ ] `make check` passes.
- [ ] Documentation/release changes also pass `make doc` and `cargo package --allow-dirty`.

## Review contract

- [ ] I completed a self-review of the final diff.
- [ ] I checked architecture review concerns: ownership, boundaries, compatibility, and resource use.
- [ ] Tests and docs cover the public behavior introduced or changed.
- [ ] Relevant task/issue status is updated without marking unverified work complete.

## Security and repository hygiene

- [ ] I assessed the security impact, including untrusted input and resource limits.
- [ ] No secrets, credentials, private exploit details, or sensitive logs are included.
- [ ] Generated files are intentional and reproducible; unrelated generated files are excluded.
- [ ] New local/build artifacts are ignored where appropriate.
