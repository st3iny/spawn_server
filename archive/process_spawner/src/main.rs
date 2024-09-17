//! As described in https://github.com/scontain/spawn_server/issues/1#issuecomment-2355368210,
//! The `spawn_server` creates this `process spawner` to reparent the target process to the
//! caller, i.e., the client.
//! This `process_spawner` thereby reparents its spawned `target_process` by setting
//! its subreaper to the `caller`, spawning the `target_process`, and subsequently exiting.
//! Thus, the spawned `target_process` reparents itself to the `caller`, allowing
//! the `caller` to `wait()` for the `target_process`.

use std::{env, ffi::CString};

use libc;


fn main() {
    let args: Vec<String> = env::args().collect();

    let caller_pid: i64 = env::var("CALLER_PID")
        .expect("`CALLER_PID` is not set in the environment")
        .parse()
        .expect("Could not parse `CALLER_PID` into an i64");

    let target_process_creator_path: std::path::PathBuf = env::var("TARGET_PROCESS_CREATOR")
        .expect("`TARGET_PROCESS_CREATOR` not set")
        .parse()
        .unwrap();

    unsafe {
        if libc::prctl(libc::PR_SET_CHILD_SUBREAPER, caller_pid) < 0 {
            let err_msg = CString::new("prctl").unwrap();
            libc::perror(err_msg.as_ptr());
            panic!("Failed to set child subreaper");
        };
    };

    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use std::env;

    use crate::main;

    #[test]
    fn test_main() {
        env::set_var("CALLER_PID", "1");

        main();
    }
}