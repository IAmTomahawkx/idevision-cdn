use std::fs;
use actix_web::{get, post, delete, middleware::Logger, error, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Error};
use actix_files::NamedFile;
use json::{parse, stringify, JsonValue};
use rand::random;
use derive_more::{Display, Error};
//use async_postgres::connect;
use qstring::QString;
use actix_web::http::{HeaderValue, StatusCode};
use lazy_static::lazy_static;

mod imaging;

lazy_static!(
    static ref CONFIG: JsonValue = parse(
    &fs::read_to_string("config.json")
        .unwrap()
    )
    .unwrap();
);

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    fs::create_dir("temp");
    std::env::set_var("RUST_LOG", "actix_web=debug");
    HttpServer::new(|| {
        App::new()
            .wrap(Logger::default())
            .service(make_img)
            .service(get_img)
    }).bind("127.0.0.1:8080")?.run().await
}

#[derive(Debug, Display, Error)]
enum Errors {
    #[display(fmt = "You are not authorized to use this route")]
    UnauthorizedRoute,

    #[display(fmt = "You are not authorized to access {}", img)]
    NoPermission { img: String },

    #[display(fmt = "Anonymous caller does not have access to protected {}", img)]
    AnonymousPermission { img: String },

    #[display(fmt = "{} cannot be found", img)]
    NotFound { img: String },

    #[display(fmt = "Misconfiguration of node {} (this is node {})", requested_node, this_node)]
    BadNode { requested_node: String, this_node: String },

    #[display(fmt = "An internal server error has occurred")]
    InternalServerError,

    #[display(fmt = err)]
    BadRequest { err: String },
}

impl error::ResponseError for Errors {
    fn error_response(&self) -> HttpResponse {
        actix_web::dev::HttpResponseBuilder::new(self.status_code())
            .set_header(actix_web::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Errors::AnonymousPermission { .. } | Errors::NoPermission { .. } | Errors::UnauthorizedRoute { .. } => StatusCode::UNAUTHORIZED,
            Errors::NotFound { .. } => StatusCode::NOT_FOUND,
            Errors::BadNode { .. } | Errors::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Errors::BadRequest { .. } => StatusCode::BAD_REQUEST,
        }
    }
}

#[get("/{node}/{image}")]
#[allow(unused_variables, unused_mut)]
async fn get_img(req: HttpRequest, path: web::Path<(String, String)>) -> Result<NamedFile, Errors> {
    let (node, image) = path.into_inner();
    if node.ne(&CONFIG["name"].to_string()) {
        return Err(Errors::BadNode { requested_node: node, this_node: CONFIG["name"].to_string() })
    }
    let mut dirlist = std::fs::read_dir("./data").unwrap();
    if dirlist.any(|p| image.eq(&p
        .as_ref()
        .unwrap()
        .file_name()
        .into_string()
        .unwrap()
    )) {
        let pth = format!("./data/{}", image);
        let mut file = NamedFile::open(pth).map_err(|_e| Errors::InternalServerError)?;
        Ok(file)
    } else {
        Err(Errors::NotFound { img: image })
    }
}

#[post("/images")]
#[allow(unused_variables, unused_mut)]
async fn make_img(req: HttpRequest, mut payload: web::Payload) -> Result<HttpResponse, Errors> {
    let query = QString::from(req.query_string());
    return if let Some(typ) = query.get(&"type") {
        if let Some(auth) = req.headers()
            .get("Authorization") {
            if auth.ne(&CONFIG["slave_key"].to_string()) {
                return Err(Errors::UnauthorizedRoute)
            }
        }

        let mut tmp_name: String = (0..10).map(|_| (0x20u8 + (random::<f32>() * 96.0) as u8) as char).collect();
        tmp_name.push_str(&*format!(".{:?}", typ));

        let mut resp = std::collections::HashMap::new();
        resp.insert("yeet", 10);
        Ok(HttpResponse::Ok().json(&resp))
    } else {
        Err(Errors::BadRequest { err: "Missing 'type' query parameter".to_string() })
    }
}
