use std::{env, error::Error, io::{self, Read, Stdout, Write}, process::{self, Stdio}, thread::sleep, time::Duration};

use num_enum::{IntoPrimitive, TryFromPrimitive};
use shared_memory::{Shmem, ShmemConf, ShmemError};


use crate::{SHMEM_FLINK, SHRUBD_ENABLE_VAR, SharedMemory};

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

    let mut buf = [0u8];
    daemon_ps.stdout.unwrap().read_exact(&mut buf).expect("Daemon didn't return succes code?");

    match StartupResult::try_from(buf[0]).expect("Daemon returned invalid result? (try restarting shrubd)") {
        StartupResult::Ok => println!("Daemon started up succesfully!"),
        StartupResult::LinkExists => eprintln!("Shared memory is already in use? continuing as if daemon was started up normally."),
        StartupResult::UnknownError => panic!("Unknown error has occured while daemon was starting up.")
    };
}

#[repr(u8)]
#[derive(TryFromPrimitive, IntoPrimitive, Debug)]
enum StartupResult {
    Ok = 0,
    UnknownError = 1,
    LinkExists = 2
}

impl From<&Box<dyn Error>> for StartupResult {
    fn from(err: &Box<dyn Error>) -> Self {
        match err.downcast_ref::<ShmemError>() {
            Some(ShmemError::LinkExists) => StartupResult::LinkExists,
            Some(_) | None => StartupResult::UnknownError
        }
    }
}

/// Daemons main function
pub(super) fn main() { 
    let shmem = match startup() {
        Ok(m) => m,
        Err(err) => {
            print!("{}", u8::from(StartupResult::from(&err)) as char);
            panic!("Fatal error occured while starting up: {}", err)
        }
    };
    
    print!("{}", u8::from(StartupResult::Ok) as char);
    let _ = io::stdout().flush();

    sleep(Duration::from_secs(10));

    shmem.as_ptr();
}

/// Starts up daemon, the end of this function signals the client to continue.
fn startup() -> Result<Shmem, Box<dyn Error>> {
    let shmem = ShmemConf::new().flink(SHMEM_FLINK).size(size_of::<SharedMemory>()).create()?;

    Ok(shmem)
}
