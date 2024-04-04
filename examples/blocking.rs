// Simple example of how to use the synchronous client API to
// talk to the spawn_server

use spawn_server::srpc;

fn main() {
    let (code, stdout, stderr) = srpc!("ls -lrt");
    println!("Blocking:\n - code={code}\n - stdout={stdout}\n - stderr={stderr}");
}
