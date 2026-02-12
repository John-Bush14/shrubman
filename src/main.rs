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
const HEARTBEAT_SHMEM_FLINK: &str = "/tmp/shared_shrubs";
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

    let heartbeat_cell = open_heartbeat_cell();
    
    println!("{:?}", *heartbeat_cell)
}


#[derive(Debug)]
pub struct Heartbeat {
    pid: Pid
}

fn open_heartbeat_cell() -> SharedMemoryCell<Heartbeat> {
    match SharedMemoryCell::<Heartbeat>::open(SHMEM_FLINK.into()) {
        Ok(cell) if cell.read().unwrap().pid.is_valid() => cell, 
        result => {
            match result {
                Ok(cell) => println!("Shared heartbeat memory contained dead daemon pid, restarting shrubd. ({:?})", cell.read().unwrap().pid),
                Err(err) => match err {
                    RcuError::SharedMemoryError(ShmemError::LinkDoesNotExist) => (),
                    RcuError::SharedMemoryError(ShmemError::LinkOpenFailed(err)) if err.kind() == ErrorKind::NotFound  => (),
                    err => eprintln!("Error occured while trying to open shared heartbeat memory ({}), attempting to restart shrubd", err),
                }
            }

            shrubd::start_shrubd();

        }
    }
}
    SharedMemoryCell::open(HEARTBEAT_SHMEM_FLINK.into()).expect("Failed to open shared heartbeat memory after shrubd has been started")
