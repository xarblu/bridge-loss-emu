use rocket::{routes, get, post};
use rocket::response::stream::ByteStream;
use rocket::data::{Data, ToByteUnit};

/// generate an infinite stream of 4KiB blocks as fast as possible
/// rng would be nice but is too slow and keeps the thread at 100% cpu
#[get("/infinite-data")]
fn infinite_data_get() -> ByteStream![[u8; 4096]] {
    ByteStream! {
        loop {
            yield [255u8; 4096];
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
    let _ = rocket::build()
        .mount("/", routes![infinite_data_get,infinite_data_post])
        .launch()
        .await;
}
