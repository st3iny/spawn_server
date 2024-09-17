use actix_web::get;
use actix_web::{post, web::Json, App, HttpResponse, HttpServer, Responder};
use shells::sh;
use spawn_server::{Command, CommandResponse};
use tokio::task;


#[post("/command")]
async fn info(command: Json<Command>) -> impl Responder {
    let cmd = command.command.clone();
    let response = if let Ok((code, stdout, stderr)) =
        task::spawn_blocking(move || sh!("{}", command.command)).await
    {
        if code != 0 {
            eprintln!("{cmd} failed: stdout='{stdout}', stderr='{stderr}'");
        }
        CommandResponse {
            code,
            stdout,
            stderr,
        }
    } else {
        eprintln!("{cmd}: failed to spawn command.");
        CommandResponse {
            code: 100,
            stdout: format!("spawn_server: command '{cmd}' failed (ERROR 128912-12128-18492)"),
            stderr: "(ERROR 128912-12128-18493)".to_string(),
        }
    };
    HttpResponse::Ok().json(response)
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(r#"{"server": "spawn_server", "version": "0.1.0"}"#)
}

fn main() {
    let _ = actix_web::rt::System::with_tokio_rt(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(16)
            .thread_name("main-tokio")
            .build()
            .unwrap()
    })
    .block_on(async_main());
}

async fn async_main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(index).service(info))
        .workers(16)
        .bind("127.0.0.1:8099")?
        .run()
        .await?;
    Ok(())
}
