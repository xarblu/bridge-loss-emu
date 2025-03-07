use rocket::{routes, get};
use rocket::response::stream::ByteStream;

/// generate an infinite stream of 4KiB blocks as fast as possible
/// rng would be nice but is too slow and keeps the thread at 100% cpu
#[get("/infinite-data")]
fn infinite_data() -> ByteStream![[u8; 4096]] {
    ByteStream! {
        loop {
            yield [255u8; 4096];
        }
    }
}

/// setup and launch rocket
pub async fn rocket_main() {
    let _ = rocket::build()
        .mount("/", routes![infinite_data])
        .launch()
        .await;
}
