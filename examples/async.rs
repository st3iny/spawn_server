// Simple example of how to use the synchronous client API to
// talk to the spawn_server

use spawn_server::arpc;

#[tokio::main]
async fn main() {
    let (code, stdout, stderr) = arpc!("ls -lrt").await;
    println!("Async:\n - code={code}\n - stdout={stdout}\n - stderr={stderr}");
}
