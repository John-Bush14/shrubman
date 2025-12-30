#![allow(irrefutable_let_patterns)]
#![allow(clippy::mut_from_ref)]

use std::{ffi::OsString, io::ErrorKind};

use shared_memory::ShmemError;

use crate::shared_rcu::{SharedRcuCell, RcuError};

mod shrubd;
mod shared_rcu;

// flink used for shared memory
const SHMEM_FLINK: &str = "/tmp/shared_shrubs";
// environment variable used to make program act as daemon
const SHRUBD_ENABLE_VAR: &str = "START_SHRUBD";

fn main() {
    if std::env::var_os(SHRUBD_ENABLE_VAR).unwrap_or(OsString::from("0")) == "1" {
        return shrubd::main();
    }

    let shmem_cell = match SharedRcuCell::<SharedMemory>::open(SHMEM_FLINK.into()) {
        Ok(m) => m,
        Err(err) => {
            match err {
                RcuError::SharedMemoryError(ShmemError::LinkDoesNotExist) => (),
                RcuError::SharedMemoryError(ShmemError::LinkOpenFailed(err)) if err.kind() == ErrorKind::NotFound  => (),
                _ => eprintln!("Error occured while trying to open shared memory, attempting to startup shrubd: {}", err),
            }

            shrubd::start_shrubd();

            SharedRcuCell::open(SHMEM_FLINK.into()).expect("Failed to open shared memory after shrubd has been started")
        }
    };

    let shmem = shmem_cell.read().unwrap();

    println!("{}", shmem.pid)
}

#[derive(Debug)]
struct SharedMemory {
    pub pid: u32
}
