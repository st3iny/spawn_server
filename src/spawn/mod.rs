pub mod spawn_client;
pub mod spawn_server;

#[derive(Debug, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct ApiPosixSpawnArgs {
    // filename (search in path -> posix_spawnp) or path (posix_spawn)
    executable: String,
    // file_actions -> see `posix_spawn(2)`
    #[serde(with = "serde_arrays")]
    file_actions: [u8; 80],
    // spawn_attr_t -> see `posix_spawn(2)`
    #[serde(with = "serde_arrays")]
    spawn_attrs: [u8; 336],
    // argv of the program
    argv: Vec<String>,
    // envp of the program
    envp: Vec<String>,
    // if set, use `posix_spawnp` ; else `posix_spawn`
    use_path: bool,
    client_pid: libc::pid_t,
}


#[cfg(test)]
mod tests {
    use std::{mem::transmute, thread};

    use actix_web::{App, HttpServer};

    use super::{spawn_client, spawn_server};

    async fn async_main() -> std::io::Result<()> {
        HttpServer::new(|| App::new().service(spawn_server::serve_posix_spawn))
            .workers(16)
            .bind("127.0.0.1:8099")?
            .run()
            .await?;
        Ok(())
    }

    // test client together with server
    // test setup:
    // - cd to the repo root
    // - `docker run -it --rm --cap-add SYS_PTRACE -v $PWD:/src rust` (also works if running as sudo on host)
    // - run the test normally via cargo inside the container
    // NOTE: the spawn server MUST be a child of the client to enable `waitpid`!
    #[actix_web::test]
    async fn test_client_server_integration() {
        let _ = thread::spawn(move || {
            let _ = actix_web::rt::System::with_tokio_rt(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(16)
                    .thread_name("main-tokio")
                    .build()
                    .unwrap()
            })
            .block_on(async_main());
        });

        let executable = "/usr/bin/cat".to_string();

        let mut file_actions: libc::posix_spawn_file_actions_t =  unsafe { transmute([0u8; 80]) };
        let mut spawn_attrs: libc::posix_spawnattr_t = unsafe { transmute([0u8; 336]) };

        if unsafe { libc::posix_spawn_file_actions_init(&mut file_actions) } < 0 {
            panic!("posix_spawn_file_actions_init: {}", std::io::Error::last_os_error());
        };

        if unsafe { libc::posix_spawnattr_init(&mut spawn_attrs) } < 0 {
            panic!("posix_spawnattr_init: {}", std::io::Error::last_os_error());
        }

        let argv = vec!["/usr/bin/cat", "/etc/hosts"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let envp = vec!["FOO=BAR"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let use_path = true;

        let spawn_pid = spawn_client::request_posix_spawn(
            executable,
            file_actions,
            spawn_attrs,
            argv,
            envp,
            use_path).await.unwrap();

        let nix_spawn_pid = nix::unistd::Pid::from_raw(spawn_pid);
        nix::sys::wait::waitpid(nix_spawn_pid, None).unwrap();
    }
}