use actix_web::{post, App, HttpResponse, HttpServer, Responder, web::Json};
use actix_web::get;
use shells::sh;
use spawn_server::{Command, CommandResponse};

#[post("/command")]
async fn info(command: Json<Command>) -> impl Responder {
    let (code, stdout, stderr) = sh!("{}", command.command);
    let response = CommandResponse { code, stdout, stderr };
    HttpResponse::Ok().json(response)
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(r#"{"server": "spawn_server", "version": "0.1.0"}"#)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(info)
    })
    .workers(4)
    .bind("127.0.0.1:8099")?
    .run()
    .await?;
    Ok(())
}
