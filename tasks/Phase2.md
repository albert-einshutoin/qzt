# Phase2: Header, Footer Trailer, and Physical Ranges

## Purpose

Implement fixed binary structures and physical range validation before variable-size blocks exist.

## Minimum MVP

```text
- Header encode/decode
- Footer Trailer encode/decode
- version, magic, length, flags, reserved bytes validation
```

## Goal MVP

```text
- physical range model implemented
- overlap detection implemented
- index_hint_offset is treated as non-authoritative
- corruption tests cover short files and invalid ranges
```

## Spec refs

```text
- Section 6.1 Physical range model
- Section 8 Fixed Header
- Section 9 Footer Trailer
- Section 35.1 Core conformance tests 22-29, 41, 48
```

## Conformance Tests Covered

```text
- Header magic, version, flags, and reserved-byte rejection
- Footer Trailer corruption rejection
- final_file_size and physical range validation
- block and reserved range overlap detection
```

## TDD Plan

Write failing tests:

```text
- valid Header round-trips
- invalid magic returns InvalidMagic
- non-zero header_flags returns InvalidFlags
- non-zero reserved bytes returns InvalidHeader
- unsupported version returns UnsupportedVersion
- file smaller than Header + Footer Trailer is rejected
- overlapping reserved ranges are rejected
```

## Implementation Tasks

```text
1. implement Header struct
2. implement FooterTrailer struct
3. implement Version type
4. implement PhysicalRange type
5. implement range overlap validator
6. add golden byte fixtures for valid fixed structures
```

## Rust Notes

Use fixed-size arrays for magic, checksums, and container IDs. Avoid string parsing for binary fields.

## Review Gates

Code review MUST be completed before this phase is marked done.

Architecture review MUST be completed before this phase is marked done.

If either review finds a spec ambiguity or library constraint, update the spec and this phase plan before continuing.

## Self-Review Checklist

```text
- Are all physical ranges half-open?
- Does every offset + size use checked_add?
- Are Header and Footer Trailer versions compared consistently?
- Are tests checking exact byte positions?
```

## Done Criteria

```text
- fixed structure tests pass
- range validation tests pass
- no variable CBOR block is trusted yet
- code review findings are fixed
- architecture review findings are fixed
- status.md is updated
```

## Status

Pending.
