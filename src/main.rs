use actix_files::NamedFile;
use actix_web::{
    get, http, middleware::Logger, post, web, web::Query, App, HttpRequest, HttpResponse,
    HttpServer,
};
use lazy_static::lazy_static;
use qstring::QString;
use rand::seq::SliceRandom;
use reqwest::Client as req_client;
use serde::Deserialize;
use std::{collections::HashMap, iter::Iterator, sync::Arc, time::Duration};
use tokio::{fs, io::AsyncWriteExt, stream::StreamExt, sync::Mutex, task, time};
use tokio_postgres::{connect, Client, NoTls};

mod errors;
mod image;
mod imaging;

use errors::Errors;

#[derive(Debug, Deserialize)]
struct Config {
    slave_key: String,
    master_site: String,
    child_site: String,
    port: u32,
    db: String,
    name: String,
    migration: bool,
    raft_propagation: Vec<String>,
}

lazy_static! {
    static ref CONFIG: Config =
        serde_json::from_str(&std::fs::read_to_string("config.json").unwrap()).unwrap();
    static ref HOLD_LOCK: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

async fn get_client() -> Client {
    let (client, conn) = connect(&CONFIG.db.as_str(), NoTls).await.unwrap();
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("connection error: {}", e);
        }
    });
    client
}

async fn find_in_cache(name: String, format: &str, size: (i32, i32)) -> String {
    let cached_name = format!("{}@{}x{}.{}", name, size.0, size.1, format);
    let mut dirlist = tokio::fs::read_dir("./cache").await.unwrap();
    if dirlist
        .any(|e| {
            e.unwrap()
                .file_name()
                .into_string()
                .unwrap()
                .eq(&cached_name)
        })
        .await
    {
        format!("./cache/{}", cached_name)
    } else {
        imaging::convert_output(format, &name, size).await
    }
}

async fn pull_from_propagation(name: &str) -> Option<(bytes::Bytes, String)> {
    if CONFIG.raft_propagation.is_empty() {
        return None;
    }
    //let e = std::path::Path::new("e");
    //let p = e.file_stem().unwrap().to_str().unwrap().to_string();
    //let c = e.
    // e.file_name()? as String;

    let client = req_client::new();
    let oks = CONFIG.raft_propagation.iter().map(|prop| {
        let client = &client;

        let url = format!(
            "https://{:}/{:}/propagation?name={:}",
            prop, &CONFIG.name, &name
        );
        (prop, async move {
            let resp = client
                .get(url)
                .header("Authorization", &CONFIG.slave_key)
                .send()
                .await
                .or(Err(()))?;
            Ok::<_, ()>(resp)
        })
    });
    for x in oks {
        let response: Result<reqwest::Response, ()> = x.1.await;
        if response.is_ok() {
            let response = response.unwrap();
            if response.status() == 200 {
                let url = format!(
                    "https://{:}/{:}/propagate?name={:}",
                    x.0, &CONFIG.name, &name
                );
                let resp = client
                    .get(url)
                    .header("Authorization", &CONFIG.slave_key)
                    .send()
                    .await;
                if resp.is_ok() {
                    let resp = resp.unwrap();
                    if resp.status().eq(&200) {
                        let data = resp.bytes().await;
                        if data.is_ok() {
                            return Some((data.unwrap(), response.text().await.unwrap()));
                        }
                    }
                }
            }
        }
    }
    None
}

fn convert_size_query(query: Query<HashMap<String, String>>) -> Result<(i32, i32), Errors> {
    if query.0.get("size").is_some() {
        let _size = query.0.get("size").unwrap().to_lowercase();
        return if _size.contains('x') {
            let (a, b) = _size.split_once('x').ok_or(Errors::BadQuery {
                query: "size".to_string(),
                reason: "Could not parse the size".to_string(),
            })?;
            let size: (i32, i32) = (a.parse().unwrap_or(-1), b.parse().unwrap_or(-1));
            if (size.0 <= 0) | (size.0 & (size.0 - 1) != 0) {
                Err(Errors::BadQuery {
                    query: "size".to_string(),
                    reason: "Could not parse width parameter (is it a power of 2?)".to_string(),
                })
            } else if (size.1 <= 0) | (size.1 & (size.1 - 1) != 0) {
                Err(Errors::BadQuery {
                    query: "size".to_string(),
                    reason: "Could not parse height parameter (is it a power of 2?)".to_string(),
                })
            } else {
                Ok(size)
            }
        } else {
            Err(Errors::BadQuery {
                query: "size".to_string(),
                reason: "Expected a size in the following format: 'WxH'. Ex. 96x96".to_string(),
            })
        };
    }
    Ok((-1, -1))
}

#[actix_web::main]
#[allow(unused_must_use)]
async fn main() {
    if fs::create_dir("temp").await.is_err() {
        assert!(
            fs::remove_dir_all("temp").await.is_ok(),
            "Failed to remove old temp dir"
        );
        assert!(
            fs::create_dir("temp").await.is_ok(),
            "Failed to create the temp dir"
        );
    }
    if fs::create_dir("cache").await.is_err() {
        assert!(
            fs::remove_dir_all("cache").await.is_ok(),
            "Failed to remove old cache dir"
        );
        assert!(
            fs::create_dir("cache").await.is_ok(),
            "Failed to create the cache dir"
        );
    }
    std::env::set_var("RUST_LOG", "actix_web=debug");
    let lock: Arc<Mutex<bool>> = Arc::clone(&HOLD_LOCK);

    task::spawn(async move {
        loop {
            time::delay_for(Duration::new(86400, 0)).await;
            //time::delay_for(Duration::new(10, 0)).await;
            let acquired = lock.lock().await;
            while Arc::strong_count(&HOLD_LOCK) > 2 {
                time::delay_for(Duration::new(0, 500)).await;
            }
            fs::remove_dir_all("./cache").await.unwrap();
            fs::create_dir("./cache").await.unwrap();
            drop(acquired);
        }
    });

    HttpServer::new(|| {
        App::new()
            .wrap(Logger::new("%a %{User-Agent}i"))
            .wrap(Logger::default())
            .wrap(
                actix_web::middleware::DefaultHeaders::new()
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Credentials", "true"),
            )
            .service(make_img)
            .service(get_img)
    })
    .bind("127.0.0.1:8080")
    .unwrap()
    .keep_alive(75)
    .run()
    .await
    .unwrap();
}

#[get("/{node}/{image:[^.]+}.{ext}")]
#[allow(unused_variables, unused_mut)]
async fn get_img(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<NamedFile, Errors> {
    let (node, image, ext) = path.into_inner();
    let query = Query::<HashMap<String, String>>::from_query(req.query_string())
        .unwrap_or_else(|_| Query(HashMap::new()));
    let size: (i32, i32) = convert_size_query(query)?;

    if node.ne(&CONFIG.name.to_string()) {
        return Err(Errors::BadNode {
            requested_node: node,
            this_node: CONFIG.name.to_string(),
        });
    }
    let mut dirlist = std::fs::read_dir("./data").unwrap();
    let as_img = image.clone() + ".webp";
    let direct = image.clone() + &*ext;

    let tlock = Arc::clone(&HOLD_LOCK);
    let lock = tlock.lock().await;
    drop(lock);

    let _image = &image;
    let _ext = &ext;
    let resp = async move {
        let exists = dirlist
            .find(|p| {
                direct.eq(&p.as_ref().unwrap().file_name().into_string().unwrap())
                    | as_img.eq(&p.as_ref().unwrap().file_name().into_string().unwrap())
            })
            .ok_or(Errors::NotFound { img: direct })??;
        let pth = format!("{}.webp", _image);
        let out = find_in_cache(pth, _ext, size).await;
        Ok::<_, Errors>(
            actix_files::NamedFile::open(out)
                .unwrap()
                .set_content_disposition(http::header::ContentDisposition {
                    disposition: http::header::DispositionType::Inline,
                    parameters: vec![],
                }),
        )
    }
    .await;
    if resp.is_ok() {
        Ok(resp.unwrap())
    } else {
        let (prop, fname) =
            pull_from_propagation(&image.as_str())
                .await
                .ok_or(Errors::NotFound {
                    img: format!("{:}.{:}", &image, &ext),
                })?;
        fs::write(format!("./data/{:}", fname), prop)
            .await
            .or::<Errors>(Err(Errors::InternalServerError)?)?;
        let out = find_in_cache(fname, &ext, size).await;
        Ok(actix_files::NamedFile::open(out)
            .unwrap()
            .set_content_disposition(http::header::ContentDisposition {
                disposition: http::header::DispositionType::Inline,
                parameters: vec![],
            }))
    }
}

#[post("/images")]
#[allow(unused_variables, unused_mut)]
async fn make_img(req: HttpRequest, mut payload: web::Payload) -> Result<HttpResponse, Errors> {
    let query = QString::from(req.query_string());
    return if let Some(typ) = query.get(&"type") {
        if let Some(auth) = req.headers().get("Authorization") {
            if auth.ne(&CONFIG.slave_key.to_string()) {
                return Err(Errors::UnauthorizedRoute);
            }
        }
        let all_names: Vec<String> = fs::read_to_string("./static/wordlist.txt")
            .await
            .unwrap()
            .lines()
            .map(|s| {
                s.to_string()
                    .insert(0, s.chars().next().unwrap().to_ascii_uppercase());
                s.to_string()
            })
            .collect();
        let new_name: String = (0..3)
            .map(|_| {
                all_names
                    .choose(&mut rand::thread_rng())
                    .unwrap()
                    .to_string()
            })
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
        Err(Errors::BadRequest {
            err: "Missing 'type' query parameter".to_string(),
        })
    };
}

#[get("/propagation")]
async fn get_propagation_exists(req: HttpRequest) -> Result<HttpResponse, HttpResponse> {
    let query = Query::<HashMap<String, String>>::from_query(req.query_string())
        .unwrap_or_else(|_| Query(HashMap::new()));

    let filename = query.0.get("name").ok_or_else(HttpResponse::NotFound)?;
    let mut dirlist = std::fs::read_dir("./data").unwrap();
    let exists = dirlist
        .find(|p| (filename.eq(&p.as_ref().unwrap().file_name().into_string().unwrap())))
        .ok_or_else(HttpResponse::NotFound)?
        .unwrap();

    Ok(HttpResponse::Ok().body(exists.file_name().into_string().unwrap()))
}

#[get("/propagate")]
async fn get_propagation_image(req: HttpRequest) -> Result<NamedFile, HttpResponse> {
    let query = Query::<HashMap<String, String>>::from_query(req.query_string())
        .unwrap_or_else(|_| Query(HashMap::new()));

    let filename = query.0.get("name").ok_or_else(HttpResponse::NotFound)?;
    let mut dirlist = std::fs::read_dir("./data").unwrap();
    let exists = dirlist
        .find(|p| (filename.eq(&p.as_ref().unwrap().file_name().into_string().unwrap())))
        .ok_or_else(HttpResponse::NotFound)?
        .unwrap();

    Ok(NamedFile::open(exists.path()).map_err(|_| HttpResponse::NotFound())?)
}
