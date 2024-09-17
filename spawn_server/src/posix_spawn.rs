use std::{ffi::CString, mem, path::PathBuf};

use actix_web::{post, web::Json, HttpResponse, Responder};
use libc::{c_char, malloc, pid_t, posix_spawn, posix_spawn_file_actions_t, posix_spawnattr_t, posix_spawnp};


#[derive(Debug, serde_derive::Deserialize, serde_derive::Serialize)]
struct PosixSpawnArgs {
    executable: String,
    file_actions: Vec<String>,
    spawnattr_t: Vec<String>,
    argv: Vec<String>,
    envp: Vec<String>,
    use_path: bool, // if set, use `posix_spawnp` ; else `posix_spawn`
}


#[post("/posix_spawn")]
async fn exec_posix_spawn(spawn_args: Json<PosixSpawnArgs>) -> impl Responder {

    let PosixSpawnArgs {
        executable,
        file_actions, // Not implementing for now; de-/serializing is not trivial
        spawnattr_t, // Not implementing for now; de-/serializing is not trivial
        argv,
        envp,
        use_path, 
    } = spawn_args.0;

    unsafe {
        let mut pid: pid_t = 0;
        let status: *mut i32 = std::ptr::null_mut();

        // Correctly allocate and populate argv
        let mut argv: Vec<*mut c_char> = argv.iter().map(|arg| arg.as_ptr() as *mut c_char).collect();
        argv.push(std::ptr::null_mut());

        // Correctly allocate and populate envp
        let mut envp: Vec<*mut c_char> = envp.iter().map(|var| var.as_ptr() as *mut c_char).collect();
        envp.push(std::ptr::null_mut());

        let exec_cstr = CString::new(executable).unwrap();


        let spawn_func = if use_path {
            posix_spawnp
        } else {
            posix_spawn
        };

        match spawn_func(&mut pid, exec_cstr.as_ptr(), std::ptr::null(), std::ptr::null(), argv.as_ptr(), envp.as_ptr()) {
            0 => (),
            _ => libc::perror("posix_spawn(p)".as_ptr() as *const i8),
        };

        if libc::waitpid(pid, status, 0) < 0 {
            libc::perror("waitpid".as_ptr() as *const i8);
        }
    };
    HttpResponse::Ok()
}


#[cfg(test)]
mod tests {
    use actix_web::{test, App};

    use super::{exec_posix_spawn, PosixSpawnArgs};

    #[actix_web::test]
    async fn test_posix_spawn() {

        let app = test::init_service(App::new().service(exec_posix_spawn)).await;

        let spawn_args = PosixSpawnArgs {
            executable:"printenv".to_string(),
            file_actions: vec![],
            spawnattr_t: vec![],
            argv: vec!["printenv".to_string()],
            envp: vec!["FOO=BAR".to_string()],
            use_path: true,
        };

        let req = test::TestRequest::post().set_json(spawn_args).uri("/posix_spawn").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let spawn_args = PosixSpawnArgs {
            executable:"/bin/ls".to_string(),
            file_actions: vec![],
            spawnattr_t: vec![],
            argv: vec!["/bin/ls".to_string(), "-a".to_string()],
            envp: vec![],
            use_path: false,
        };

        let req = test::TestRequest::post().set_json(spawn_args).uri("/posix_spawn").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
