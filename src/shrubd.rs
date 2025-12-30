use std::{env, fs::{self}, io::{self, Read, Write}, process::{self, Stdio}, thread::sleep, time::Duration};

use num_enum::{IntoPrimitive, TryFromPrimitive};
use shared_memory::ShmemError;


use crate::{Pid, SHMEM_FLINK, SHRUBD_ENABLE_VAR, SharedMemory, shared_rcu::{RcuError, SharedRcuCell}};

/// Starts shrubd, waits for it's succes code and then disowns it
pub(super) fn start_shrubd() {
    println!("Starting daemon...");

    let self_path = env::current_exe().unwrap();

     #[allow(clippy::zombie_processes)]
    let daemon_ps = process::Command::new(self_path)
        .current_dir("/")
        .env(SHRUBD_ENABLE_VAR, "1")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Shrubd command failed to start.");
    
    sleep(Duration::from_secs(1));
    let mut buf = [0u8];
    daemon_ps.stdout.unwrap().read_exact(&mut buf).expect("Daemon didn't return succes code?");

    match StartupResult::try_from(buf[0]).expect("Daemon returned invalid result? (try restarting shrubd)") {
        StartupResult::Ok => println!("Daemon started up succesfully!"),
        StartupResult::Error => panic!("Unknown error has occured while daemon was starting up.")
    };
}

#[repr(u8)]
#[derive(TryFromPrimitive, IntoPrimitive, Debug)]
enum StartupResult {
    Ok = 0,
    Error = 1,
}
impl StartupResult {fn return_result(self) {
    print!("{}", u8::from(self) as char); 
    let _ = io::stdout().flush();
}}


/// Daemons main function
pub(super) fn main() { 
    let shmem_cell = create_shmem_cell();

    let _ = shmem_cell.write(SharedMemory { pid: Pid(process::id() as _) });
    
    StartupResult::Ok.return_result();

    sleep(Duration::from_secs(10));
}

fn create_shmem_cell() -> SharedRcuCell<SharedMemory> {
    match SharedRcuCell::<SharedMemory>::create(SHMEM_FLINK.into()) {
        Ok(c) => c,
        Err(RcuError::SharedMemoryError(ShmemError::LinkExists)) => {
            fs::remove_file(SHMEM_FLINK).expect("Link exists but doesn't exist?"); 
            create_shmem_cell()
        }
        Err(err) => {
            StartupResult::Error.return_result();
            panic!("Fatal error occured while starting up: {}", err)
        }
    }
}
