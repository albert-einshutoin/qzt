# Fixture Strategy

Fixtures are split by trust level:

```text
source/   Original UTF-8 inputs used to build valid QZT containers.
valid/    Well-formed QZT containers generated from source fixtures.
corrupt/  Intentionally malformed containers for parser and verifier tests.
```

Later phases should keep generated binary fixtures small, deterministic, and documented by the test that consumes them.
