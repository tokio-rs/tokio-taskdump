// Copyright (c) 2014 Alex Crichton
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// the MIT license <LICENSE-MIT>, at your option.
//
// Original source: https://github.com/rust-lang/backtrace-rs/blob/master/src/backtrace/libunwind.rs

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use core::ffi::c_void;

#[repr(C)]
pub(crate) enum _Unwind_Reason_Code {
    _URC_NO_REASON = 0,
    _URC_END_OF_STACK = 5,
    _URC_FAILURE = 9,
}

pub(crate) enum _Unwind_Context {}

pub(crate) type _Unwind_Trace_Fn =
    extern "C" fn(ctx: *mut _Unwind_Context, arg: *mut c_void) -> _Unwind_Reason_Code;

unsafe extern "C" {
    pub(crate) fn _Unwind_Backtrace(
        trace: _Unwind_Trace_Fn,
        trace_argument: *mut c_void,
    ) -> _Unwind_Reason_Code;
    pub(crate) fn _Unwind_GetIP(ctx: *mut _Unwind_Context) -> usize;
    pub(crate) fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void;
}
