use std::{
    ffi::CString, os::raw::c_void
};

use actix_web::{post, web::Json, Responder};
use libc::{
    c_char, pid_t, posix_spawn_file_actions_t
};

use super::ApiPosixSpawnArgs;


#[repr(C)]
#[derive(Debug, Clone)]
struct CloneProcessSpawnerArgs {
    pid: *mut pid_t,
    executable: *const libc::c_char,
    file_actions_p: *const libc::posix_spawn_file_actions_t,
    spawn_attrs: *const libc::posix_spawnattr_t,
    argv: *const *mut libc::c_char,
    envp: *const *mut libc::c_char,
    client_pid: pid_t,
    use_path: bool,
    target_pid_pipe: [libc::c_int; 2],
}


#[repr(C)]
struct PosixFileActions {
    __allocated: libc::c_int,
    __used: libc::c_int,
    __actions: *mut libc::c_int,
    __pad: [libc::c_int; 16],
}


const FDOP_CLOSE: libc::c_int = 1;
const FDOP_DUP2: libc::c_int = 2;
const FDOP_OPEN: libc::c_int = 3;

const FILENO_OFFSET: i32 = 1000;
const NEW_STDIN_FILENO: i32 = FILENO_OFFSET + libc::STDOUT_FILENO;
const NEW_STDOUT_FILENO: i32 = FILENO_OFFSET + libc::STDOUT_FILENO;
const NEW_STDERR_FILENO: i32 = FILENO_OFFSET + libc::STDERR_FILENO;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Fdop {
    next: *const Fdop,
    prev: *const Fdop,
    cmd: libc::c_int,
    fd: libc::c_int,
    srcfd: libc::c_int,
    oflag: libc::c_int,
    mode: libc::mode_t,
    path: *const libc::c_char,
}


extern "C" fn target_process(arg: *mut c_void) -> libc::c_int {

    // some work is required to emulate `posix_spawn` via the server
    // besides reparenting, which we describe above the server `clone` call,
    // the `spawner` (being the process executing this function) must retrieve
    // certain file descriptors from the calling `client`.
    // 1. The spawner must get the stdin/out/err of the client.
    // 2. The spawner perform certain actions to emulate file_actions.

    let process_spawner_args_p: *const CloneProcessSpawnerArgs = unsafe { std::mem::transmute_copy(&arg) };
    let process_spawner_args = unsafe { (*process_spawner_args_p).clone() };

    let CloneProcessSpawnerArgs {
        pid,
        executable,
        file_actions_p,
        spawn_attrs,
        argv,
        envp,
        client_pid,
        use_path,
        target_pid_pipe,
    } = process_spawner_args;


    // 1. The spawner must get the stdin/out/err of the client.
    // Requires getting fds of the client (which requires PTRACE_MODE_ATTACH_REALCREDS -> see ptrace(2); achievable via docker or sudo)

    // get the client's pidfd
    let client_pid_fd = unsafe {
        libc::syscall(libc::SYS_pidfd_open, client_pid, 0)
    };

    if client_pid_fd < 0 {
        panic!("pidfd_open: {}", std::io::Error::last_os_error());
    };

    // move own FDs (0, 1, 2) and replace w/ client FDs
    for (std_fd, new_fd) in vec![(libc::STDIN_FILENO, NEW_STDIN_FILENO), (libc::STDOUT_FILENO, NEW_STDOUT_FILENO), (libc::STDERR_FILENO, NEW_STDERR_FILENO)] {
        // get the client's std*
        let client_fd: i32 = unsafe {
            libc::syscall(libc::SYS_pidfd_getfd, client_pid_fd, std_fd, 0)
                .try_into()
                .unwrap()
        };
        if client_fd < 0 {
            panic!("pidfd_getfd: {}", std::io::Error::last_os_error());
        }

        // save own std* to new fd
        let _ = nix::unistd::dup2(std_fd, new_fd)
            .expect("dup2: Failed to dup stdin/out/err");

        // close own std*
        nix::unistd::close(std_fd).expect("close: Failed to close stdin/out/err");

        // dup client's std*
        nix::unistd::dup2(client_fd, std_fd).unwrap();

        // close source client's std*
        nix::unistd::close(client_fd).unwrap();
    }


    // 2. The spawner perform certain actions to emulate file_actions.
    // transmute posix_spawn.__actions to own file actions object st. we can access 'private' fields
    let file_actions = if !file_actions_p.is_null() {
        let c_file_actions = unsafe { *file_actions_p };
        Some(unsafe { std::mem::transmute::<posix_spawn_file_actions_t, PosixFileActions>(c_file_actions) })
    } else {
        None
    };

    // adapted from musl-libc: https://github.com/esmil/musl/blob/master/src/process/posix_spawn.c
    if let Some(file_actions) = file_actions {
        let mut file_actions_op_p: *const Fdop = unsafe { std::mem::transmute::<*mut libc::c_int, *const Fdop>(file_actions.__actions) };
        while !file_actions_op_p.is_null() {
            let op = unsafe { *file_actions_op_p };
            
            match op.cmd {
                // if FD is not 0,1,2: no action required; simply don't get the FD from the client
                // otherwise: close the FD
                FDOP_CLOSE => {
                    match op.fd {
                        libc::STDIN_FILENO | libc::STDOUT_FILENO | libc::STDERR_FILENO => {
                            nix::unistd::close(op.fd).unwrap();
                        }
                        _ => (),
                    }
                },
                // no action required; `posix_spawn` can open any path on its own, without
                // any further info required from the client
                FDOP_OPEN => {},
                // get the client fd, then dup2 it
                FDOP_DUP2 => {
                    let client_fd: i32 = unsafe {
                        libc::syscall(libc::SYS_pidfd_getfd, client_pid_fd, op.srcfd, 0)
                            .try_into()
                            .unwrap()
                    };

                    if client_fd < 0 {
                        panic!("pidfd_getfd: {}", std::io::Error::last_os_error());
                    }

                    let _ = nix::unistd::dup2(client_fd, op.fd).unwrap();

                    // close no longer needed fd to avoid collisions with further `dup`s
                    nix::unistd::close(client_fd).unwrap();
                },
                n => panic!("Got invalid fdop cmd: {n}"),
            }

            file_actions_op_p = unsafe {
                let faop = *file_actions_op_p;
                faop.next
            };
        }
    }

    // let c_str = CString::new("TARGET PID: %d\n").unwrap();
    // let c_str_2 = CString::new("TARGET PPID: %d\n").unwrap();
    // unsafe { libc::printf(c_str.as_ptr(), libc::getpid()); };
    // unsafe { libc::printf(c_str_2.as_ptr(), libc::getppid()); };

    // unsafe { libc::execvpe(executable, ptr_array, envp.as_ptr()) };
    let spawn_func = if use_path {
        libc::posix_spawn
    } else {
        libc::posix_spawnp
    };

    
    if unsafe { spawn_func(
        pid, executable, file_actions_p, spawn_attrs, argv, envp
    ) } < 0 {
        panic!("posix_spawn: {}", std::io::Error::last_os_error())
    };


    if unsafe { *pid } < 0 {
        panic!("posix_spawn: {}", std::io::Error::last_os_error())
    }

    // communicate created PID back to calling process, which communicates again back to client
    nix::unistd::close(target_pid_pipe[0]).unwrap();
    unsafe {
        if libc::write(
            target_pid_pipe[1],
            pid as *mut c_void,
            std::mem::size_of::<*mut pid_t>()
        ) as usize != std::mem::size_of::<*mut pid_t>() {
            panic!("write: '{}'", std::io::Error::last_os_error());
        }
    };

    // let cstr1 = CString::new("TARGET PID %d\n").unwrap();
    // let cstr2 = CString::new("TARGET PARENT PID %d\n").unwrap();

    // let pid1 = unsafe { libc::getpid() };
    // let pid2 = unsafe { libc::getppid() };

    // unsafe { libc::printf(cstr1.as_ptr(), pid1); };
    // unsafe { libc::printf(cstr2.as_ptr(), pid2); };

    0
}


fn exec_posix_spawn(spawn_args: ApiPosixSpawnArgs) -> (libc::pid_t, libc::pid_t) {
    let ApiPosixSpawnArgs {
        executable,
        file_actions, // Not implementing for now; de-/serializing is not trivial
        spawn_attrs: spawnattr_t,  // Not implementing for now; de-/serializing is not trivial
        argv,
        envp,
        use_path,
        client_pid,
    } = spawn_args;

    let mut stack: [libc::c_int; 1024] = [0; 1024];
    let spawner_pid: pid_t = 0;
    let mut target_pid: pid_t = 0;
    let mut pipe_fds: [libc::c_int; 2] = [0i32; 2];

    let executable = CString::new(executable).unwrap();
    let file_actions =  unsafe { std::mem::transmute::<[u8; 80], libc::posix_spawn_file_actions_t>(file_actions) };
    let spawn_attrs = unsafe { std::mem::transmute::<[u8; 336], libc::posix_spawnattr_t>(spawnattr_t) };

    // Correctly allocate and populate argv
    let mut argv: Vec<*mut c_char> = argv.iter().map(|arg| arg.as_ptr() as *mut c_char).collect();
    argv.push(std::ptr::null_mut());

    // Correctly allocate and populate envp
    let mut envp: Vec<*mut c_char> = envp.iter().map(|var| var.as_ptr() as *mut c_char).collect();
    envp.push(std::ptr::null_mut());

    if unsafe {
        libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_DIRECT)
    } < 0 {
        panic!("pipe2: {}", std::io::Error::last_os_error());
    };

    let process_spawner_args = CloneProcessSpawnerArgs {
        pid: &mut target_pid,
        executable: executable.as_ptr(),
        file_actions_p: &file_actions,
        spawn_attrs: &spawn_attrs,
        argv: argv.as_ptr(),
        envp: envp.as_ptr(),
        client_pid,
        use_path,
        target_pid_pipe: pipe_fds,
    };


    if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1) } < 0 {
        panic!("prctl {}", std::io::Error::last_os_error());
    }

    // using libc::clone instead of nix::sched::clone so we can do C trickery to pass `arg` and the pipe therein
    // could maybe also be done w/ `nix`, however, easier to do for now w/ `libc::clone`
    //
    // IMPORTANT: reparenting of `posix_spawn` process
    // GOAL: process `spawned` by `spawner` of the server should have same parent as server (which is the client)
    // - client spawns the server -> hence, parent of server is client
    // - server `clone`s `spawner` process (which performs the actual `posix_spawn`)
    //   thereby sets:
    //     - `SIGCHLD` -> necessary for server to `wait` for `spawner` child (fork in musl is implemented as clone w/ SIGCHLD)
    //     - `CLONE_PARENT_SETTID` -> server waits for child (`spawner`) to complete (which does not wait for its child) NOTE: client may have to wait for both!!!
    //     - `PR_SET_CHILD_SUBREAPER` -> server's children are reparented to client
    // NOTE: server MUST be called by client for reparenting to work!
    let ret = unsafe {
        libc::clone(
            target_process,
            (stack.as_mut_ptr().wrapping_add(1024)) as *mut libc::c_void,
            // libc::SIGCHLD | libc::CLONE_PARENT | libc::CLONE_PARENT_SETTID,
            // TODO: keep or replace VFORK?
            libc::SIGCHLD | libc::CLONE_PARENT_SETTID,
            (&process_spawner_args as *const CloneProcessSpawnerArgs) as *mut libc::c_void,
            &spawner_pid,
        )
    };

    if ret < 0 { panic!("clone: '{}'", std::io::Error::last_os_error()); };

    nix::unistd::close(pipe_fds[1]).unwrap();


    unsafe {
        if libc::read(
            pipe_fds[0],
            (&mut target_pid as *mut pid_t) as *mut c_void,
            std::mem::size_of::<*mut pid_t>()
        ) as usize != std::mem::size_of::<*mut pid_t>() {
            panic!("read: '{}'", std::io::Error::last_os_error());
        }
    };

    (spawner_pid, target_pid)
}


#[post("/posix_spawn")]
async fn serve_posix_spawn(spawn_args: Json<ApiPosixSpawnArgs>) -> actix_web::Result<impl Responder> {
    let (spawner_pid, target_pid) = exec_posix_spawn(spawn_args.0);
    Ok(Json((spawner_pid, target_pid)))
}


#[cfg(test)]
mod tests {
    use std::vec;

    use actix_web::App;

    use super::{exec_posix_spawn, serve_posix_spawn, ApiPosixSpawnArgs};

    fn debug_pid_args() -> ApiPosixSpawnArgs {
        let client_pid = unsafe { libc::getpid() };

        ApiPosixSpawnArgs {
            executable: "/src/debug-programs/pid".to_string(),
            file_actions: [0u8; 80],
            spawn_attrs: [0u8; 336],
            argv: vec!["pid".to_string()],
            envp: vec![],
            use_path: true,
            client_pid,
        }

    }

    fn create_ls_spawnargs() -> ApiPosixSpawnArgs {
        let client_pid = unsafe { libc::getpid() };

        ApiPosixSpawnArgs {
            executable: "ls".to_string(),
            file_actions: [0u8; 80],
            spawn_attrs: [0u8; 336],
            argv: vec!["ls".to_string(), "-a".to_string()],
            envp: vec![],
            use_path: false,
            client_pid,
        }
    }


    #[test]
    fn test_exec_posix_spawn() {

        dbg!("client pid:", nix::unistd::getpid());

        // let spawn_args = create_ls_spawnargs();
        let spawn_args = debug_pid_args();
        let (spawner_pid, target_pid) = exec_posix_spawn(spawn_args);
        assert!(spawner_pid > 0, "expected spawner pid > 0, got {}", target_pid);
        assert!(target_pid > 0, "expected target pid > 0, got {}", target_pid);

        // dbg!("caller sees target pid as:", target_pid);
        // dbg!("callers PID: ", unsafe { libc::getpid() });

        // idea: target_process SIGTRAPS itself
        // client calls `pctrl(PR_SET_TRACER, target_pid)` to allow target process to get fds
        // client signals continue to target process
        // target process gets fds -> `pidfd_getfd(2)`: duplicates fd (AND SETS FD_CLOEXEC)
        // -> hence: need to `dup(2)` FDs we wish to keep open after execve -> do together w/ posix spawn

        // let mut wstatus: libc::c_int = 0;
        // assert_ne!(unsafe {
        //     libc::waitpid(target_pid, &mut wstatus, 0)
        // }, -1, "waitpid: {}", std::io::Error::last_os_error());

        // assert!(libc::WIFEXITED(wstatus));
        // assert_eq!(libc::WEXITSTATUS(wstatus), 0);

        
        let spawner_nix_pid: nix::unistd::Pid = nix::unistd::Pid::from_raw(spawner_pid);
        let target_nix_pid: nix::unistd::Pid = nix::unistd::Pid::from_raw(target_pid);
        nix::sys::wait::waitpid(Some(spawner_nix_pid), None).unwrap();
        nix::sys::wait::waitpid(Some(target_nix_pid), None).unwrap();
    }

    #[actix_web::test]
    async fn test_posix_spawn_server() {
        use actix_web::test;

        let app = test::init_service(App::new()
        .service(serve_posix_spawn)).await;

        let spawn_args = create_ls_spawnargs();

        let req = test::TestRequest::post()
            .set_json(spawn_args)
            .uri("/posix_spawn")
            .to_request();
        let res = test::call_service(&app, req).await;

        assert!(&res.status().is_success());

        let (spawner_pid, target_pid): (i32, i32) = test::read_body_json(res).await;

        let spawner_nix_pid: nix::unistd::Pid = nix::unistd::Pid::from_raw(spawner_pid);
        let target_nix_pid: nix::unistd::Pid = nix::unistd::Pid::from_raw(target_pid);
        nix::sys::wait::waitpid(Some(spawner_nix_pid), None).unwrap();
        nix::sys::wait::waitpid(Some(target_nix_pid), None).unwrap();
    }
}
