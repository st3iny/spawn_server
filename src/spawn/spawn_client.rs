use anyhow::{bail, Context};

use super::ApiPosixSpawnArgs;


const SPAWN_SERVER_HARDCODED_URL: &str = "http://localhost:8099/posix_spawn";

pub async fn request_posix_spawn(
    // filename (search in path -> posix_spawnp) or path (posix_spawn)
    executable: String,
    // file_actions -> see `posix_spawn(2)`
    file_actions: libc::posix_spawn_file_actions_t,
    // spawn_attr_t -> see `posix_spawn(2)`
    spawn_attrs: libc::posix_spawnattr_t,
    // argv of the program
    argv: Vec<String>,
    // envp of the program
    envp: Vec<String>,
    // if set, use `posix_spawnp` ; else `posix_spawn`
    use_path: bool,
) -> anyhow::Result<libc::pid_t> {

    let file_actions_b: [u8; 80] = unsafe { std::mem::transmute(file_actions) };
    let spawnattr_b: [u8; 336] = unsafe { std::mem::transmute(spawn_attrs) };
    let pid = nix::unistd::getpid();

    let api_spawn_args = ApiPosixSpawnArgs {
        executable,
        file_actions: file_actions_b,
        spawn_attrs: spawnattr_b,
        argv,
        envp,
        use_path,
        client_pid: pid.as_raw(),
    };

    let client = reqwest::Client::new();
    let res = client.post(SPAWN_SERVER_HARDCODED_URL)
        .json(&api_spawn_args)
        .send()
        .await
        .context("Failed to send `posix_spawn` request to spawn server")?;

    if res.status().is_client_error() {
        bail!("server-side error: '{}'", res.status());
    }

    let (spawner_pid, target_pid) = res.json::<(i32, i32)>()
        .await
        .context("Client received different JSON body than expected.")?;

    let nix_spawner_pid = nix::unistd::Pid::from_raw(spawner_pid);

    let wait_status = nix::sys::wait::waitpid(nix_spawner_pid, None)
        .context("Could not wait for the posix_spawn spawner of the server to terminate")?;

    if let nix::sys::wait::WaitStatus::Exited(_, status) = wait_status {
        if status != 0 {
            bail!("server-side posix-spawn returned non-zero exit code: {status}");
        }
    } else {
        bail!("Expected posix_spawn spawner to exit; instead, received wait status: {:?}", wait_status);
    };

    Ok(target_pid)
}