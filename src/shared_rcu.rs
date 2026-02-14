//! Lock-free RCU (Read-Copy-Update) cell implementation over POSIX shared memory.
//!
//! This module provides [`SharedRcuCell`], a concurrent data structure that allows
//! wait-free reads and atomic writes across process boundaries using shared memory.
//!
//! # Design
//!
//! The RCU cell maintains N redundant copies of data in a circular buffer. Writers
//! atomically update an offset pointer after writing new data, while readers access
//! the current slot without blocking.
//!
//! # Safety
//!
//! The implementation validates pointers and offsets before dereferencing and should
//! be completely safe, except for if more than N writes happen before a read.

use std::{ffi::OsString, ops::Deref, sync::atomic::{AtomicUsize, Ordering}};
use shared_memory::{Shmem, ShmemConf, ShmemError};
use thiserror::Error;

/// A lock-free RCU cell backed by POSIX shared memory with N-way redundancy.
///
/// # Type Parameters
///
/// - `T`: The data type stored in the cell
/// - `N`: Number of redundant copies (how many writes can happen before a read becomes unsafe)
pub struct SharedRcuCell<T: Sized, const N: usize> {
    _shmem_handle: Shmem,
    shmem_ptr: *mut SharedMemory<T, N>,
}

#[repr(C)]
pub struct SharedMemory<T: Sized, const N: usize> {
    values: [T; N],
    offset: AtomicUsize
}

impl<T, const N: usize> Deref for SharedRcuCell<T, N> {
    type Target = T;

    // I know ok, if there was a way to deref to a result I would do it.
    fn deref(&self) -> &Self::Target {self.read().expect("Dereferenced invalid shared memory")}
}

impl<T, const N: usize> SharedRcuCell<T, N> {
    pub fn read(&self) -> Result<&T, RcuError> {
        self.check_shmem()?;

        unsafe {
            Ok(&*(self.gptr()))
        }   
    }

    const fn shmem(&self) -> &mut SharedMemory<T, N> {unsafe {&mut *self.shmem_ptr}}

    fn gptr(&self) -> *mut T {unsafe {self.shmem_ptr.add(self.offset()) as *mut T}}

    fn offset(&self) -> usize {self.shmem().offset.load(Ordering::Relaxed)}

    pub fn check_shmem(&self) -> Result<(), RcuError> {
        match (self.shmem_ptr.is_null(), !self.shmem_ptr.is_aligned()) {
            (false, false) => (),
            (n, a) => return Err(RcuError::InvalidShmemPtr(n, a))
        };

        if self.offset() > const {(N-1) * Self::T_SIZE}
            || !self.offset().is_multiple_of(Self::T_SIZE) 
        {
            return Err(RcuError::InvalidOffset(self.offset()))
        }

        let gptr = self.gptr();

        match (gptr.is_null(), !gptr.is_aligned()) {
            (false, false) => (),
            (n, a) => return Err(RcuError::InvalidGptr(n, a))
        };

        Ok(())
    }

    const T_SIZE: usize = size_of::<T>();

    pub fn write(&self, data: T) -> Result<(), RcuError> {
        self.check_shmem()?;

        let new_offset = (self.offset() + Self::T_SIZE) % const {N * Self::T_SIZE};

        unsafe {
            let new_gptr = self.shmem_ptr.add(new_offset) as *mut T;

            new_gptr.write(data);
            
            self.shmem().offset.swap(new_offset, Ordering::Release);
        }

        Ok(())
    }

    pub fn open(flink: OsString) -> Result<Self, RcuError> {
        let shmem_handle = match ShmemConf::new().flink(flink).open() {
            Ok(m) => m,
            Err(err) => return Err(RcuError::SharedMemoryError(err))
        };

        let s = Self::new(shmem_handle);

        s.check_shmem()?; 

        Ok(s)
    }

    pub fn create(flink: OsString) -> Result<Self, RcuError> {
        let shmem_size = size_of::<SharedMemory<T, N>>();

        let shmem_handle = match ShmemConf::new().flink(flink).size(shmem_size).create() {
            Ok(m) => m,
            Err(err) => return Err(RcuError::SharedMemoryError(err))
        };

        unsafe {
            let shmem = &mut *(shmem_handle.as_ptr() as *mut SharedMemory<T, N>);

            shmem.offset = AtomicUsize::from(0);
        }
        
        Ok(Self::new(shmem_handle))
    }

    fn new(shmem_handle: Shmem) -> Self {Self {shmem_ptr: shmem_handle.as_ptr() as _, _shmem_handle: shmem_handle}}
}

#[derive(Error, Debug)]
pub enum RcuError {
    #[error("Shared memory pointer was null ({0}) or misaligned ({1})")]
    InvalidShmemPtr(bool, bool),
    #[error("Global pointer in shared memory was null ({0}) or misaligned ({1})")]
    InvalidGptr(bool, bool),
    #[error("Global pointer offset is an invalid value ({0})")]
    InvalidOffset(usize),
    #[error("Shared memory error occured (at opening or creation): {0}")]
    SharedMemoryError(ShmemError)
}
