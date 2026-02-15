use std::{env, fs::{self}, io::{self, Read, Write}, process::{self, Stdio}, thread::sleep, time::Duration};

use inplace_containers::InplaceString;
use log::{info, warn};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use shared_memory::ShmemError;


use crate::{DAEMON_STATUS_SHMEM_FLINK, DaemonStatus, Pid, SHRUBD_ENABLE_VAR, SharedMemoryCell, shared_rcu::RcuError};

/// Starts shrubd, waits for it's succes code and then disowns it
pub(super) fn start_shrubd() {
    info!("Starting daemon...");

    let self_path = env::current_exe().unwrap();

     #[allow(clippy::zombie_processes)]
    let daemon_ps = process::Command::new(self_path)
        .env(SHRUBD_ENABLE_VAR, "1")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Shrubd command failed to start.");
    
    sleep(Duration::from_secs(1));
    let mut buf = [0u8];
    match daemon_ps.stdout.unwrap().read_exact(&mut buf) {
        Ok(_) => (),
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => (), // will be handled with StartupResult::NoResult
        Err(err) => panic!("Failed to read startup result from daemon: {}", err)
    };

    match StartupResult::try_from(buf[0]).expect("Daemon returned invalid result? (try restarting shrubd)") {
        StartupResult::Ok => info!("Daemon started up succesfully!"),
        err => panic!("Unknown error has occured while daemon was starting up. {:?}", err)
    };
}

#[repr(u8)]
#[derive(TryFromPrimitive, IntoPrimitive, Debug)]
enum StartupResult {
    NoResult = 0,
    Ok = 1,
    Error = 2,
}

impl StartupResult {fn return_result(self) {
    print!("{}", u8::from(self) as char); 
    let _ = io::stdout().flush();
}}


/// Daemons main function
pub(super) fn main() {
    env_logger::init();

    let daemon_status = create_daemon_status();

    let mut version = InplaceString::new(); version.push_str(env!("CARGO_PKG_VERSION"));
    let _ = daemon_status.write(DaemonStatus { pid: Pid(process::id() as _), version });
    
    StartupResult::Ok.return_result();

    sleep(Duration::from_secs(10));
}

fn create_daemon_status() -> SharedMemoryCell<DaemonStatus> {
    match SharedMemoryCell::create(DAEMON_STATUS_SHMEM_FLINK.into()) {
        Ok(c) => c,
        Err(RcuError::SharedMemoryError(ShmemError::LinkExists)) => {
            fs::remove_file(DAEMON_STATUS_SHMEM_FLINK).expect("Link exists but doesn't exist?"); 
            create_daemon_status()
        }
        Err(err) => {
            StartupResult::Error.return_result();
            panic!("Fatal error occured while starting up: {}", err)
        }
    }
}
