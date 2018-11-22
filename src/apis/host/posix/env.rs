/// NOTE: These syscalls only support wasm_32 for now because they take u32 offset

use libc::{
    c_int,
    c_void,
    size_t,
    ssize_t,
    exit,
    read,
    open,
    close,
};
use std::os::raw::c_char;
use std::ffi::CStr;

use crate::webassembly::{Instance};
use std::env;

pub extern "C" fn get_env(name: &str, instance: &mut Instance) -> Result<String, env::VarError> {
    debug!("host::get_env({:?})", name);
    env::var(name)
}
