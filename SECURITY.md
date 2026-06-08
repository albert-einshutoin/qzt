# Security Policy

## Supported Versions

Only the current `main` branch and the latest tagged technical preview receive
security fixes. Pre-`v1.0` APIs and performance characteristics may still
change, but QZT v0.1 container bytes are treated as compatibility-sensitive
once format stability is declared.

## Reporting a Vulnerability

Please report suspected vulnerabilities privately by opening a GitHub security
advisory for `albert-einshutoin/qzt`.

If advisories are unavailable, contact the repository owner through the GitHub
profile linked from the repository. Include a minimal reproducer, expected
impact, and whether the issue affects parsing untrusted `.qzt` containers.

Public issues are appropriate for non-sensitive hardening ideas, but not for
exploitable parser, decompression, or resource-exhaustion reports.
