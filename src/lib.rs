mod jit;
mod syms;

#[macro_use]
extern crate lazy_static;

use crate::jit::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::os::raw::c_int;
use std::os::raw::c_long;
use std::os::raw::c_void;

#[repr(C)]
pub struct JitTarget {
    target_function: *const c_void,
    num_args: c_int,
    jitted: Option<*const u8>,
}

#[repr(C)]
pub struct Dl_info {
    dli_fname: *const c_void,
    dli_fbase: *const c_void,
    dli_sname: *const c_char,
    dli_saddr: *const c_void,
}

extern "C" {
    pub fn dladdr(addr: *const c_void, info: *mut Dl_info) -> c_int;
}

pub fn get_sym_name(func: *const c_void) -> String {
    let mut info = Dl_info {
        dli_fname: std::ptr::null(),
        dli_fbase: std::ptr::null(),
        dli_sname: std::ptr::null(),
        dli_saddr: std::ptr::null(),
    };
    unsafe {
        dladdr(func, &mut info);
    }
    if info.dli_sname.is_null() {
        panic!("could not find {:?}", func);
    }
    unsafe { CStr::from_ptr(info.dli_sname) }
        .to_str()
        .unwrap()
        .to_string()
}

#[no_mangle]
pub extern "C" fn initializeJIT(verbosity: c_int) {
    println!("initializeJIT verbosity: {}", verbosity);
}

#[no_mangle]
pub extern "C" fn loadBitcode(c_file_name: *const c_char) {
    let file_name = unsafe { CStr::from_ptr(c_file_name) };
    load_bitcode(file_name.to_str().unwrap().to_string())
}

#[no_mangle]
pub extern "C" fn createJitTarget(
    target_function: *const c_void,
    num_args: c_int,
) -> Box<JitTarget> {
    let jit_target = JitTarget {
        target_function,
        num_args,
        jitted: None,
    };
    println!(
        "creating JitTarget for: addr: {:?} name: {}",
        target_function,
        get_sym_name(target_function)
    );
    Box::new(jit_target)
}

fn get_code(jit_target: &mut JitTarget) -> Option<*const u8> {
    if jit_target.jitted.is_none() {
        let start = std::time::Instant::now();
        jit_target.jitted = Some(jit_func(get_sym_name(jit_target.target_function)).unwrap());
        println!("took {:?} to JIT", start.elapsed());
    }
    jit_target.jitted
}

#[no_mangle]
pub extern "C" fn runJitTarget0(jit_target: &mut JitTarget) -> c_long {
    call_func(get_code(jit_target).unwrap(), &[])
}

#[no_mangle]
pub extern "C" fn runJitTarget1(jit_target: &mut JitTarget, arg0: c_long) -> c_long {
    call_func(get_code(jit_target).unwrap(), &[arg0 as u64])
}
