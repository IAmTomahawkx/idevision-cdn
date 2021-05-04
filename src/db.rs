use async_postgres::{connect, Client, Config};

async fn get() -> Client {
    let client = Client("postgres://tom:cba321@127.0.0.1:5432/test").await?;
    return client
}
