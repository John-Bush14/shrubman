use std::{ffi::OsString, marker, sync::atomic::{AtomicUsize, Ordering}};
use shared_memory::{Shmem, ShmemConf, ShmemError};
use thiserror::Error;


pub struct SharedRcuCell<T> {
    _shmem_handle: Shmem,
    shmem_ptr: *mut (AtomicUsize, T, T),
    _marker: marker::PhantomData<T>
}

impl<T> SharedRcuCell<T> {
    pub fn read(&self) -> Result<&T, RcuError> {
        self.check_shmem()?;

        unsafe {
            Ok(&*(self.gptr()))
        }   
    }

    const fn shmem(&self) -> &mut (AtomicUsize, T, T) {unsafe {&mut *self.shmem_ptr}}

    fn gptr(&self) -> *mut T {unsafe {self.shmem_ptr.add(self.shmem().0.load(Ordering::Relaxed)) as *mut T}}

    fn check_shmem(&self) -> Result<(), RcuError> {
        match (self.shmem_ptr.is_null(), !self.shmem_ptr.is_aligned()) {
            (false, false) => (),
            (n, a) => return Err(RcuError::InvalidShmemPtr(n, a))
        };

        let gptr = self.gptr();

        match (gptr.is_null(), !gptr.is_aligned()) {
            (false, false) => (),
            (n, a) => return Err(RcuError::InvalidGptr(n, a))
        };

        Ok(())
    }

    pub fn write(&self, data: T) -> Result<(), RcuError> {
        self.check_shmem()?;

        let offset = self.shmem().0.load(Ordering::Relaxed);

        let t_size = size_of::<T>();
        let new_offset = match offset - size_of::<AtomicUsize>() {
            0 => t_size,
            _ if offset == t_size => 0,
            _ => return Err(RcuError::InvalidOffset(offset))
        } + size_of::<AtomicUsize>();

        unsafe {
            let new_gptr = self.shmem_ptr.add(new_offset) as *mut T;

            new_gptr.write(data);
            
            self.shmem().0.swap(new_offset, Ordering::Release);
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
        let shmem_size = size_of::<(AtomicUsize, T, T)>();

        let shmem_handle = match ShmemConf::new().flink(flink).size(shmem_size).create() {
            Ok(m) => m,
            Err(err) => return Err(RcuError::SharedMemoryError(err))
        };

        unsafe {
            let shmem = &mut *(shmem_handle.as_ptr() as *mut (AtomicUsize, T, T));

            shmem.0 = AtomicUsize::from(size_of::<AtomicUsize>());
        }
        
        Ok(Self::new(shmem_handle))
    }

    fn new(shmem_handle: Shmem) -> Self {Self {shmem_ptr: shmem_handle.as_ptr() as _, _shmem_handle: shmem_handle, _marker: marker::PhantomData}}
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
