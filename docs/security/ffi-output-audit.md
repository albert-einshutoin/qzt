# CLI output FFI audit (Issue #307)

This audit covers the ten `rust.lang.security.unsafe-usage.unsafe-usage`
findings carried from #31. The baseline is main
`2db7b23432513acddcc3b85e2d143c764660dfd2` on 2026-09-26. The
`semgrep/semgrep:1.166.0` image had digest
`sha256:c180f0c93a17b420c0af5006214a29d3c747c5459c732b740191adf657dd0068`.
From a clean checkout, the workflow command
`semgrep scan --config p/rust --error --metrics=off .` scanned 112 files with
11 rules, reported exactly ten blocking findings, and exited 1. This was a
completed scan, not a scanner startup or network failure. The scanner image,
ruleset name, `.semgrepignore`, and security workflow are unchanged by this
work; the remote `p/rust` contents are not locally pinned.

## Call-site decisions

`preserve_output_security` is called only after the CLI has rejected an
existing-output symlink, an input/output collision, and a Windows readonly
destination. `create_atomic_output` has created a new temporary file in the
destination directory. A failure in the security copy aborts before output
replacement and triggers temporary-file cleanup. The caller repeats collision
checks before replacement. Normal permissions are copied separately; this
table covers the ACL/DACL FFI and the final Windows move. Every exception below
is one exact-rule `nosemgrep` at its call site, beside a `SAFETY` explanation;
none exempts a file or another `unsafe` call.

| OS and call | Safety conditions and result handling | Decision and evidence |
| --- | --- | --- |
| macOS `fcopyfile` | The source and temporary `File` values keep both `int` descriptors live. The SDK's `copyfile.h` defines flag `1` as `COPYFILE_ACL` and requires a NULL state in the current implementation. Only ACL metadata is requested; a negative return is an error and `errno` is captured immediately. | Fix `== -1` to `< 0` per the API contract; retain this FFI with one audited exception. macOS ACL-preservation integration test. |
| Linux `fgetxattr`, length | The source owns the live fd. `c"system.posix_acl_access"` is NUL-terminated. A NULL value pointer and size zero request the current length; `-1` is handled before any allocation. The Linux POSIX ACL path explicitly returns `ENODATA` for an absent ACL; it removes a default ACL inherited by the temp. `ENOTSUP` means the same filesystem cannot expose POSIX ACLs, and other errors abort. | Retain with one audited exception. Linux ACL and parent-default-ACL tests. |
| Linux `fremovexattr` | The temporary `File` owns the live fd; the name remains live and terminated. Removing a missing attribute (`ENODATA`) is acceptable; all other errors abort. The operation is reached only when the existing output's access ACL is absent. | Retain with one audited exception. Linux parent-default-ACL test. |
| Linux `fgetxattr`, value | The first result is converted to `usize` before allocation. The zero-initialized vector supplies exactly its reported writable byte length; both fd and name remain live. A changed length, `ERANGE`, or any other failed second read aborts before use or replacement. | Retain with one audited exception. Linux ACL-preservation test. |
| Linux `fsetxattr` | Only the successful second read's initialized bytes are passed with their exact length. The temporary fd and name remain live. A failed write aborts before replacement. | Retain with one audited exception. Linux ACL-preservation test. |
| Windows `GetFileSecurityW`, size query | The source UTF-16 vector is NUL-terminated and live. The descriptor argument is NULL with length zero, and `needed` is a writable `DWORD`. A failed probe now proceeds only for `ERROR_INSUFFICIENT_BUFFER`; other OS errors are returned immediately. `needed` must be nonzero and at most 1 MiB. | Fix probe error checking; retain with one audited exception. Windows protected/unprotected DACL and readonly tests. |
| Windows `GetFileSecurityW`, descriptor read | `needed` is converted safely and rounded up to native `usize` words, bounded by 1 MiB. The initialized vector has native pointer alignment and enough writable bytes. The source path and length output remain live. Only a successful call provides the self-relative descriptor passed onward. | Fix `Vec<u32>` alignment to `Vec<usize>`; retain with one audited exception. Windows DACL tests. |
| Windows `GetSecurityDescriptorControl` | The preceding read succeeded and produced a complete self-relative descriptor. Its native-aligned buffer remains live; `control` (`WORD`) and `revision` (`DWORD`) are writable. Failure is returned before either value is used. | Retain with one audited exception. Windows protected/unprotected DACL tests. |
| Windows `SetFileSecurityW` | The complete self-relative descriptor and NUL-terminated temporary path remain live. The flags select DACL only and preserve whether the source DACL is protected. The process must have `WRITE_DAC` or ownership; OS refusal aborts replacement. | Retain with one audited exception. Windows DACL tests. |
| Windows `MoveFileExW` | Both UTF-16 paths are NUL-terminated and live. The temp was created beside the output, so the move stays on one volume. `0x1` requests replacement and `0x8` requests write-through; copy-across-volume is not enabled. A zero return is converted to the immediate OS error. | Retain with one audited exception. Windows safe-output and fault-injection tests. |

The Windows descriptor change addresses an API precondition: Microsoft states
that security descriptors used by several functions require a valid native
pointer boundary, which a `Vec<u32>` does not promise on 64-bit systems.
Current allocators may happen to over-align it, so an integration test passing
with the old vector would not establish the guarantee. The new vector's
element type provides native pointer alignment. Successful `GetFileSecurityW`
returns a self-relative descriptor; no raw descriptor bytes are interpreted in
Rust. The `u32` size outputs and `u16` control match the Win32 `DWORD` and
`SECURITY_DESCRIPTOR_CONTROL` types. The FFI declarations use `extern "system"`
on Windows and `extern "C"`/`libc` on Unix.

Linux routes `system.posix_acl_access` through `do_get_acl` and `vfs_get_acl`,
which propagate LSM errors, return `EOPNOTSUPP` when POSIX ACLs are unsupported,
and return `ENODATA` when `__get_acl` reports no ACL. This is narrower than the
generic `getxattr(2)` description, where `ENODATA` can also indicate an
inaccessible attribute. A custom filesystem or LSM that deliberately masks
an ACL access denial as `ENODATA` cannot be distinguished from absence here;
the CLI's ACL-preservation guarantee does not extend to that behavior.

## Detection and regression evidence

The same container scan after the source changes reports zero findings and
exits 0. This means ten **local reviewed exceptions**, not that the functions
are memory-safe Rust or that the rule has been disabled. A separate, tracked
one-file temporary Git repository containing an unannotated Rust `unsafe`
expression produced one finding with the exact rule ID and exit 1. Adding
only `// nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage` before that
expression produced zero findings and exit 0. The temporary repository was
removed; no maintained test framework or scan exclusion was added.

The focused CLI tests are `cargo test --test phase44_safe_file_output --
--nocapture` and `cargo test --bin qzt atomic_output_tests`. The PR's existing
macOS, Linux, and Windows CI jobs must be checked for test execution rather
than counting a platform skip as success. The workflow still runs Semgrep with
`--error`, OSV on `Cargo.lock` with `fail-on-vuln: true`, and Gitleaks over
full history. No security workflow or `.semgrepignore` change is made.

The output contract remains input self-overwrite and symlink rejection,
Windows readonly rejection before temporary output, preservation of the old
output and input on pre-replacement failures, and ACL/DACL preservation for a
successful replacement. Linux's inherited parent default ACL is removed when
the old output has no access ACL. This audit does not establish Windows
same-volume directory-entry durability after a power loss, nor preservation
of file ownership, unrelated extended attributes, or security labels.

## Primary specifications

- [Apple `copyfile(3)` / `fcopyfile(3)`](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/copyfile_state_alloc.3.html) and the installed macOS SDK `copyfile.h` for `COPYFILE_ACL` and the negative-error return.
- Linux [`getxattr(2)`](https://man7.org/linux/man-pages/man2/getxattr.2.html), [`setxattr(2)`](https://man7.org/linux/man-pages/man2/setxattr.2.html), [`removexattr(2)`](https://man7.org/linux/man-pages/man2/removexattr.2.html), and kernel [`vfs_get_acl`](https://github.com/torvalds/linux/blob/master/fs/posix_acl.c) and [`do_getxattr`](https://github.com/torvalds/linux/blob/master/fs/xattr.c) implementations.
- Microsoft [`GetFileSecurityW`](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getfilesecurityw), [`SECURITY_DESCRIPTOR` alignment](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-security_descriptor), [`GetSecurityDescriptorControl`](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getsecuritydescriptorcontrol), [`SetFileSecurityW`](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-setfilesecurityw), [`SECURITY_INFORMATION`](https://learn.microsoft.com/en-us/windows/win32/secauthz/security-information), and [`MoveFileExW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).
