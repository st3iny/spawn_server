use serde_derive::{Deserialize, Serialize};

pub mod spawn;

use spawn::spawn_client;
use spawn::spawn_server;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Command {
    pub command: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CommandResponse {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

fn get_spawner_url() -> String {
    "http://localhost:8099/command".to_string()
}

pub fn sync_remote_execute(cmd: &str) -> (i32, String, String) {
    let cmd = Command {
        command: cmd.to_string(),
    };

    use std::time::Duration;
    let very_long_timeout = Duration::new(60 * 60 * 24, 0); // 24h

    let client = reqwest::blocking::Client::builder()
        .timeout(very_long_timeout)
        .build()
        .unwrap(); // todo: remove unwrap

    match client.post(get_spawner_url()).json(&cmd).send() {
        Ok(resp) => {
            if resp.status().is_success() {
                let result: CommandResponse = resp.json().unwrap();
                (result.code, result.stdout, result.stderr)
            } else {
                eprintln!("sync_remote_execute API: return status indicates no success (ERROR 8192-173-10620)");
                (
                    -2,
                    "".to_string(),
                    "No Success Error (ERROR 8192-173-10620)".to_string(),
                )
            }
        }
        Err(e) => {
            eprintln!(
                "sync_remote_execute API response cannot be parsed! {} (ERROR 67132-2323-78223)",
                e
            );
            (
                -1,
                "".to_string(),
                "RPC Error  (ERROR 67132-2323-78224)".to_string(),
            )
        }
    }
}

//  async_version

pub async fn remote_execute(cmd: &str) -> (i32, String, String) {
    let cmd = Command {
        command: cmd.to_string(),
    };

    use std::time::Duration;
    let very_long_timeout = Duration::new(60 * 60 * 24, 0);

    let client = reqwest::Client::builder()
        .timeout(very_long_timeout)
        .build()
        .unwrap(); // todo: remove unwrap

    match client.post(get_spawner_url()).json(&cmd).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let result: CommandResponse = resp.json().await.unwrap();
                (result.code, result.stdout, result.stderr)
            } else {
                eprintln!("async_remote_execute API: return status indicates no success (ERROR 8192-173-10620)");
                (
                    -2,
                    "".to_string(),
                    "No Success Error (ERROR 8192-173-10620)".to_string(),
                )
            }
        }
        Err(e) => {
            eprintln!(
                "async_remote_execute API response cannot be parsed! {} (ERROR 67132-2323-78123)",
                e
            );
            (
                -1,
                "".to_string(),
                "RPC Error  (ERROR 67132-2323-78124)".to_string(),
            )
        }
    }
}

/// Macro to execute the given command on the spawn server using synchronous communcation

#[macro_export]
macro_rules! srpc {
    ( $( $cmd:tt )* ) => {{
        $crate::sync_remote_execute(&format!($( $cmd )*))
    }};
}

/// Macro to execute the given command on the spawn server using asynchronous communcation

#[macro_export]
macro_rules! arpc {
    ( $( $cmd:tt )* ) => {{
        $crate::remote_execute(&format!($( $cmd )*))
    }};
}

#[macro_export]
macro_rules! sh {
    ( $( $cmd:tt )* ) => {{
        $crate::sync_remote_execute(&format!($( $cmd )*))
    }};
}
