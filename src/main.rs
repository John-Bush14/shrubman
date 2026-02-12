#![allow(irrefutable_let_patterns)]
#![allow(clippy::mut_from_ref)]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

use std::{ffi::OsString, fs, io::ErrorKind};

use shared_memory::ShmemError;

use crate::shared_rcu::{SharedRcuCell, RcuError};

mod shrubd;
mod shared_rcu;


// flink used for shared memory
const SHMEM_FLINK: &str = "/tmp/shared_shrubs";
// environment variable used to make program act as daemon
const SHRUBD_ENABLE_VAR: &str = "START_SHRUBD";

const REDUNDANCY: usize = 3;
type SharedMemoryCell<T> = SharedRcuCell<T, REDUNDANCY>;

#[derive(Debug)]
struct Pid(libc::pid_t);
impl Pid {fn is_valid(&self) -> bool {
    fs::exists(format!("/proc/{}", self.0)).unwrap_or(false)
}}


fn main() {
    if std::env::var_os(SHRUBD_ENABLE_VAR).unwrap_or(OsString::from("0")) == "1" {
        return shrubd::main();
    }

    let general_shmem_cell = open_general_shmem_cell();
    
    println!("{:?}", *general_shmem_cell)
}


#[derive(Debug)]
struct GeneralSharedMemory {
    pub pid: Pid
}

fn open_general_shmem_cell() -> SharedMemoryCell<GeneralSharedMemory> {
    match SharedMemoryCell::<GeneralSharedMemory>::open(SHMEM_FLINK.into()) {
        Ok(cell) if cell.read().unwrap().pid.is_valid() => cell, 
        result => {
            match result {
                Ok(cell) => println!("Shared memory contained dead daemon pid, restarting shrubd. ({:?})", cell.read().unwrap().pid),
                Err(err) => match err {
                    RcuError::SharedMemoryError(ShmemError::LinkDoesNotExist) => (),
                    RcuError::SharedMemoryError(ShmemError::LinkOpenFailed(err)) if err.kind() == ErrorKind::NotFound  => (),
                    err => eprintln!("Error occured while trying to open shared memory ({}), attempting to restart shrubd", err),
                }
            }

            shrubd::start_shrubd();

            SharedMemoryCell::open(SHMEM_FLINK.into()).expect("Failed to open shared memory after shrubd has been started")
        }
    }
}
