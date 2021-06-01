use actix_web::{
    web::Query,
    get,
    post,
    middleware::Logger,
    web,
    App,
    http,
    HttpRequest,
    HttpResponse,
    HttpServer,
    dev::{
        ServiceResponse,
    }
};
use json::{
    parse,
    JsonValue
};
use tokio_postgres::{
    connect,
    Client,
    NoTls
};
use futures::future::{
    Ready
};
use actix_files::NamedFile;
use rand::seq::SliceRandom;
use qstring::QString;
use lazy_static::lazy_static;
use std::{
    collections::HashMap,
    sync::Arc
};
use errors::Errors;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::stream::StreamExt;

mod imaging;
mod errors;

lazy_static!(
    static ref CONFIG: JsonValue = parse(
    &std::fs::read_to_string("config.json")
        .unwrap()
    )
    .unwrap();
    static ref FS_LOCK: Arc<bool> = Arc::new(false);
);

async fn get_client() -> Client {
    let (client, conn) = connect(&CONFIG["dsn"].as_str().unwrap(), NoTls).await.unwrap();
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("connection error: {}", e);
        }
    });
    client
}

async fn find_in_cache(name: String, format: String, size: (i32, i32)) -> String {
    let cached_name = format!("{}@{}x{}.{}", name, size.0, size.1, format);
    let mut dirlist = tokio::fs::read_dir("./cache").await.unwrap();
    if dirlist.any(|e| e.unwrap().file_name().into_string().unwrap().eq(&cached_name)).await {
        format!("./cache/{}", cached_name)
    } else {
        imaging::convert_output(&format, &name, size).await
    }
}

fn convert_size_query(query: Query<HashMap<String, String>>) -> Result<(i32, i32), Errors> {
    if query.0.get("size").is_some() {
        let _size = query.0.get("size").unwrap().to_lowercase();
        return if _size.contains('x') {
            let (a, b) = _size.split_once('x').ok_or(Errors::BadQuery { query: "size".to_string(), reason: "Could not parse the size".to_string() })?;
            let size: (i32, i32) = (
                a.parse().unwrap_or(-1),
                b.parse().unwrap_or(-1)
            );
            if (size.0 <= 0) | (size.0 & (size.0 - 1) != 0) {
                Err(Errors::BadQuery { query: "size".to_string(), reason: "Could not parse width parameter (is it a power of 2?)".to_string() })
            } else if (size.1 <= 0) | (size.1 & (size.1 - 1) != 0) {
                Err(Errors::BadQuery { query: "size".to_string(), reason: "Could not parse height parameter (is it a power of 2?)".to_string() })
            } else {
                Ok(size)
            }
        } else {
            Err(Errors::BadQuery { query: "size".to_string(), reason: "Expected a size in the following format: 'WxH'. Ex. 96x96".to_string() })
        }
    }
    Ok((-1, -1))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _t = fs::create_dir("temp").await.is_ok();
    let _t = fs::create_dir("cache").await.is_ok();

    std::env::set_var("RUST_LOG", "actix_web=debug");

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::new("%a %{User-Agent}i"))
            .wrap(Logger::default())
            .wrap(actix_web::middleware::DefaultHeaders::new()
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Credentials", "true")
            )
            .service(make_img)
            .service(get_img)
    }).bind("127.0.0.1:8080")?
        .keep_alive(75)
        .run()
        .await
}

#[get("/{node}/{image:[^.]+}.{ext}")]
#[allow(unused_variables, unused_mut)]
async fn get_img(req: HttpRequest, path: web::Path<(String, String, String)>) -> Result<NamedFile, Errors> {
    let (node, image, ext) = path.into_inner();
    let query = Query::<HashMap<String, String>>::from_query(req.query_string())
        .unwrap_or(Query(HashMap::new()));
    let size: (i32, i32) = convert_size_query(query)?;

    println!("{:?}/{:?}", image, ext);
    if node.ne(&CONFIG["name"].to_string()) {
        return Err(Errors::BadNode { requested_node: node, this_node: CONFIG["name"].to_string() })
    }
    let mut dirlist = std::fs::read_dir("./data").unwrap();
    let as_img = image.clone() + ".webp";
    let direct = image.clone() + &*ext;

    let exists = dirlist.find(|p|
        (direct.eq(&p
                .as_ref()
                .unwrap()
                .file_name()
                .into_string()
                .unwrap())
            | as_img.eq(&p
            .as_ref()
            .unwrap()
            .file_name()
            .into_string()
            .unwrap()))
    ).ok_or(Errors::NotFound { img: direct})?;
    if exists.is_ok() {
        let pth = format!("{}.webp", image);
        let out = find_in_cache(pth, ext, size).await;
        Ok(actix_files::NamedFile::open(out).unwrap().set_content_disposition(http::header::ContentDisposition {disposition: http::header::DispositionType::Inline, parameters: vec![]}))
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
        let all_names: Vec<String> = fs::read_to_string("./static/wordlist.txt").await
            .unwrap()
            .lines()
            .map(|s| {
                s.to_string().insert(0, s.chars().next().unwrap().to_ascii_uppercase());
                s.to_string()
            })
            .collect();
        let new_name: String = (0..3).map(|_| all_names.choose(&mut rand::thread_rng())
            .unwrap()
            .to_string())
            .collect();

        let mut tmp_name = "./temp/".to_string();
        tmp_name.push_str(&new_name);
        tmp_name.push_str(&*format!(".{}", typ));

        let mut resp: HashMap<&str, &str> = std::collections::HashMap::new();
        let mut file = fs::File::create(&tmp_name).await.unwrap();

        while let Some(chunk) = payload.next().await {
            file.write_all(&chunk?).await.unwrap();
        }
        file.flush().await.expect("failed to flush");
        imaging::convert_intake(&typ, &tmp_name, &new_name).await;
        resp.insert("name", &new_name);
        Ok(HttpResponse::Ok().json(&resp))
    } else {
        Err(Errors::BadRequest { err: "Missing 'type' query parameter".to_string() })
    }
}
