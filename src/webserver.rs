use std::net::Ipv4Addr;
use rocket::{get, post, routes};
use rocket::response::stream::ByteStream;
use rocket::data::{Data, ToByteUnit};

// 4MiB chunk size
const CHUNK_SIZE: usize = 4 * 1024 * 1024;

/// generate an infinite stream of chunks as fast as possible
/// rng would be nice but is too slow and keeps the thread at 100% cpu
#[get("/infinite-data")]
fn infinite_data_get() -> ByteStream![[u8; CHUNK_SIZE]] {
    ByteStream! {
        loop {
            yield [255u8; CHUNK_SIZE];
        }
    }
}

/// accept an "infinite" stream of data
#[post("/infinite-data", data =  "<data>")]
async fn infinite_data_post(data: Data<'_>) -> Result<(), String> {
    let _ = data.open(64.tibibytes())
        .stream_to(tokio::io::sink()).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// setup and launch rocket
pub async fn rocket_main() {
    let cfg = rocket::config::Config {
        address: Ipv4Addr::new(0,0,0,0).into(),
        limits: rocket::data::Limits::default()
            .limit("data", 64.mebibytes()
        ),
        ..rocket::config::Config::default()
    };

    let _ = rocket::custom(cfg)
        .mount("/", routes![infinite_data_get,infinite_data_post])
        .launch()
        .await;
}
