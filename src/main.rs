#![allow(irrefutable_let_patterns)]
#![allow(clippy::mut_from_ref)]
#![allow(incomplete_features)]
#![feature(adt_const_params)]

use std::{error::Error, ffi::OsString, fs};

use inplace_containers::InplaceString;
use thiserror::Error;

use crate::shared_rcu::SharedRcuCell;

mod shrubd;
mod shared_rcu;


// flink used for shared memory
const HEARTBEAT_SHMEM_FLINK: &str = "/tmp/shared_shrubs";
// environment variable used to make program act as daemon
const SHRUBD_ENABLE_VAR: &str = "START_SHRUBD";

const REDUNDANCY: usize = 3;
type SharedMemoryCell<T> = SharedRcuCell<T, REDUNDANCY>;

/// 11 should be more than enough.
type VersionString = InplaceString<11>;

#[derive(Debug, Clone, Copy)]
pub struct Pid(libc::pid_t);
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
    pid: Pid,
    version: VersionString
}

fn open_heartbeat_cell() -> SharedMemoryCell<Heartbeat> {
    let restart_reason: Box<dyn Error> = match SharedMemoryCell::<Heartbeat>::open(HEARTBEAT_SHMEM_FLINK.into()) {
        Ok(cell) => {
            if let Err(err) = cell.read().unwrap().is_beating() {
                Box::new(err)
            } else {return cell}
        }, 
        Err(err) => Box::new(err),
    };

    eprintln!("Shrubd is assumed not to be alive or valid because off ({}), attempting to restart shrubd", restart_reason);

    shrubd::start_shrubd();

    SharedMemoryCell::open(HEARTBEAT_SHMEM_FLINK.into()).expect("Failed to open shared heartbeat memory after shrubd has been started")
}

impl Heartbeat {
    fn is_beating(&self) -> Result<(), CardiacArrest> {
        let version = env!("CARGO_PKG_VERSION");
        if self.version != version {
            eprintln!("Daemon is running a different version ({}) to current running process ({}), unintended behaviour (probably just segfaults) might ensue.", self.version, version);
        }

        if !self.pid.is_valid() {return Err(CardiacArrest::DeadPid(self.pid))}

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum CardiacArrest {
    #[error("Shrubd is dead (pid = {0:?})")]
    DeadPid(Pid),  
}

