#![allow(irrefutable_let_patterns)]


use std::{ffi::OsString, io::{ErrorKind}};

use shared_memory::{ShmemConf, ShmemError};

mod shrubd;

// flink used for shared memory
const SHMEM_FLINK: &str = "/tmp/shared_shrubs";
// environment variable used to make program act as daemon
const SHRUBD_ENABLE_VAR: &str = "START_SHRUBD";

fn main() {
    if std::env::var_os(SHRUBD_ENABLE_VAR).unwrap_or(OsString::from("0")) == "1" {
        return shrubd::main();
    }

    let shmem = match ShmemConf::new().flink(SHMEM_FLINK).open() {
        Ok(m) => m,
        Err(err) => {
            match err {
                ShmemError::LinkDoesNotExist => (),
                ShmemError::LinkOpenFailed(err) if err.kind() == ErrorKind::NotFound  => (),
                _ => eprintln!("Non-fatal error occured while trying to open shared memory: {}", err),
            }

            shrubd::start_shrubd();

            ShmemConf::new().flink(SHMEM_FLINK).open().expect("Failed to open shared memory after shrubd has been started")
        }
    };
}

struct SharedMemory {
    pid: u64
}
