use rocket::fs::FileServer;
use std::path::Path;

pub async fn host_file(file: String) -> () {
    let path = Path::new(file.as_str())
        .canonicalize().unwrap();

    // stable rocket doesn't support hosting individual files yet
    let dir_name = path.parent().unwrap();
    let server = FileServer::from(dir_name);
    let _ = rocket::build()
        .mount("/", server)
        .launch()
        .await;
}
