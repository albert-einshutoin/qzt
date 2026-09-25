use std::fs::File;
use std::io;
use std::path::Path;

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub(super) fn preserve_output_security(
    existing: &Path,
    _temp_path: &Path,
    temp: &File,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    #[link(name = "System")]
    unsafe extern "C" {
        fn fcopyfile(from: i32, to: i32, state: *mut std::ffi::c_void, flags: u32) -> i32;
    }

    let source = File::open(existing)?;
    // COPYFILE_ACL copies access control without copying file data.
    // SAFETY: Both File values keep their descriptors open for the call. NULL
    // state is supported, and flag 1 is COPYFILE_ACL in copyfile.h.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited fcopyfile FFI; see docs/security/ffi-output-audit.md.
    if unsafe {
        fcopyfile(
            source.as_raw_fd(),
            temp.as_raw_fd(),
            std::ptr::null_mut(),
            1,
        )
    } < 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub(super) fn preserve_output_security(
    existing: &Path,
    _temp_path: &Path,
    temp: &File,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;

    let source = File::open(existing)?;
    let name = c"system.posix_acl_access";
    // SAFETY: source owns the live fd, name is NUL-terminated, and a NULL
    // value with size 0 asks the kernel for the attribute length.
    let length =
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited fgetxattr length query; see docs/security/ffi-output-audit.md.
        unsafe { libc::fgetxattr(source.as_raw_fd(), name.as_ptr(), std::ptr::null_mut(), 0) };
    if length == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENODATA) {
            // A new temp may inherit its parent's default ACL even when the
            // existing output has none. Remove that inherited access ACL.
            // SAFETY: temp owns the live fd and name is a NUL-terminated C
            // string; fremovexattr does not retain either argument.
            // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited fremovexattr call; see docs/security/ffi-output-audit.md.
            if unsafe { libc::fremovexattr(temp.as_raw_fd(), name.as_ptr()) } == -1 {
                let remove_error = io::Error::last_os_error();
                if remove_error.raw_os_error() != Some(libc::ENODATA) {
                    return Err(remove_error);
                }
            }
            return Ok(());
        }
        if error.raw_os_error() == Some(libc::ENOTSUP) {
            return Ok(());
        }
        return Err(error);
    }
    let mut acl = vec![0_u8; usize::try_from(length).map_err(io::Error::other)?];
    // SAFETY: source and name remain live; acl provides acl.len() writable
    // bytes. A changed size fails below instead of exposing partial data.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited fgetxattr value read; see docs/security/ffi-output-audit.md.
    let actual = unsafe {
        libc::fgetxattr(
            source.as_raw_fd(),
            name.as_ptr(),
            acl.as_mut_ptr().cast(),
            acl.len(),
        )
    };
    if actual != length {
        return Err(if actual == -1 {
            io::Error::last_os_error()
        } else {
            io::Error::other("output ACL changed during read")
        });
    }
    // SAFETY: temp owns the live fd; acl has exactly the initialized bytes
    // reported by the successful read, and name is NUL-terminated.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited fsetxattr call; see docs/security/ffi-output-audit.md.
    if unsafe {
        libc::fsetxattr(
            temp.as_raw_fd(),
            name.as_ptr(),
            acl.as_ptr().cast(),
            acl.len(),
            0,
        )
    } == -1
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub(super) fn preserve_output_security(
    existing: &Path,
    temp_path: &Path,
    _temp: &File,
) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn GetFileSecurityW(
            path: *const u16,
            information: u32,
            descriptor: *mut std::ffi::c_void,
            length: u32,
            needed: *mut u32,
        ) -> i32;
        fn SetFileSecurityW(
            path: *const u16,
            information: u32,
            descriptor: *mut std::ffi::c_void,
        ) -> i32;
        fn GetSecurityDescriptorControl(
            descriptor: *mut std::ffi::c_void,
            control: *mut u16,
            revision: *mut u32,
        ) -> i32;
    }

    let source: Vec<u16> = existing.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
    const DACL: u32 = 0x4;
    const ERROR_INSUFFICIENT_BUFFER: i32 = 122;
    let mut needed = 0_u32;
    // SAFETY: source is NUL-terminated and remains live, needed is writable,
    // and GetFileSecurityW permits a NULL descriptor with zero length here.
    let probe =
        // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited GetFileSecurityW size query; see docs/security/ffi-output-audit.md.
        unsafe { GetFileSecurityW(source.as_ptr(), DACL, std::ptr::null_mut(), 0, &mut needed) };
    if probe == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER) {
            return Err(error);
        }
    }
    if needed == 0 || needed > 1024 * 1024 {
        return Err(io::Error::other(
            "cannot read bounded output security descriptor",
        ));
    }
    let word_count = usize::try_from(needed)
        .map_err(io::Error::other)?
        .div_ceil(std::mem::size_of::<usize>());
    let mut descriptor = vec![0_usize; word_count];
    let descriptor_ptr = descriptor.as_mut_ptr().cast();
    // SAFETY: the usize vector has native pointer alignment and at least
    // needed writable bytes; source and needed remain live for the call.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited GetFileSecurityW descriptor read; see docs/security/ffi-output-audit.md.
    if unsafe { GetFileSecurityW(source.as_ptr(), DACL, descriptor_ptr, needed, &mut needed) } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut control = 0_u16;
    let mut revision = 0_u32;
    // SAFETY: GetFileSecurityW succeeded, so descriptor_ptr names a complete
    // self-relative descriptor; control and revision are writable scalars.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited GetSecurityDescriptorControl call; see docs/security/ffi-output-audit.md.
    if unsafe { GetSecurityDescriptorControl(descriptor_ptr, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let protection = if control & 0x1000 != 0 {
        0x8000_0000
    } else {
        0x2000_0000
    };
    // SAFETY: descriptor remains live and initialized after the successful
    // read; destination is NUL-terminated and live; neither is retained.
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- audited SetFileSecurityW call; see docs/security/ffi-output-audit.md.
    if unsafe { SetFileSecurityW(destination.as_ptr(), DACL | protection, descriptor_ptr) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
