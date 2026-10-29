use std::{
    ffi::c_void,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};

use anyhow::{Context as _, ensure};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SDDL_REVISION_1,
        },
        DACL_SECURITY_INFORMATION, GetTokenInformation, OWNER_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, SetFileSecurityW, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

struct LocalBuffer(*mut c_void);

impl Drop for LocalBuffer {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn checked(result: i32, owner: &'static str) -> anyhow::Result<()> {
    if result == 0 {
        return Err(io::Error::last_os_error()).context(owner);
    }
    Ok(())
}

fn current_sid() -> anyhow::Result<String> {
    let mut token = ptr::null_mut();
    checked(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) },
        "SSH storage: cannot read the Windows access token",
    )?;
    let _token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
    }
    ensure!(
        needed > 0,
        "SSH storage: Windows returned an empty access token"
    );
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    checked(
        unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        },
        "SSH storage: cannot read the Windows owner's SID",
    )?;
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = ptr::null_mut();
    checked(
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) },
        "SSH storage: cannot encode the Windows owner's SID",
    )?;
    let _sid = LocalBuffer(sid.cast());
    let mut length = 0;
    while unsafe { *sid.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(sid, length) })
        .context("SSH storage: Windows returned an invalid owner SID")
}

pub(super) fn protect(path: &Path, folder: bool) -> anyhow::Result<()> {
    let sid = current_sid()?;
    let inheritance = if folder { "OICI" } else { "" };
    let descriptor: Vec<u16> = format!("O:{sid}D:P(A;{inheritance};FA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut security = ptr::null_mut();
    checked(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                descriptor.as_ptr(),
                SDDL_REVISION_1,
                &mut security,
                ptr::null_mut(),
            )
        },
        "SSH storage: cannot build a private Windows ACL",
    )?;
    let _security = LocalBuffer(security);
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    checked(
        unsafe {
            SetFileSecurityW(
                path.as_ptr(),
                OWNER_SECURITY_INFORMATION
                    | DACL_SECURITY_INFORMATION
                    | PROTECTED_DACL_SECURITY_INFORMATION,
                security,
            )
        },
        "SSH storage: cannot restrict the private path to the current Windows user",
    )
}
