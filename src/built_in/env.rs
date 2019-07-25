use std::ffi::CString;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;

#[allow(non_snake_case)]
pub fn getPATH() -> Option<CString> {
    let val = std::env::var_os("PATH")?;
    let mut s = OsString::from("PATH=");
    s.push(val);
    Some(CString::new(s.into_vec()).unwrap())
}
