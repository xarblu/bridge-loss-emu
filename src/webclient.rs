use futures::StreamExt;
use std::time::{SystemTime, Duration};

/**
 * Downloader that fetches a stream of data from url
 * printing stats about the transfer to the console
 * @param url  Url to download from
 */
pub async fn download(url: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("[webclient] Downloading from {}", url);

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap();

    let mut stream = response.bytes_stream();

    let mut cur_time = SystemTime::now();
    let mut cur_bytes: u64 = 0;
    while let Some(item) = stream.next().await {
        let chunk = item.expect("[webclient] Download failed");
        cur_bytes += chunk.len() as u64;

        // every ~5 seconds print status
        let elapsed = cur_time.elapsed().unwrap();
        if elapsed >= Duration::from_secs(5) {
            // rate in mbit/s
            let rate = ((((cur_bytes as f64) * 8.0) / 1000.0) / 1000.0) / elapsed.as_secs_f64();
            println!("[webclient] Downloaded {} MB in {}s at rate {} Mbit/s",
                cur_bytes / 1000 / 1000,
                elapsed.as_secs_f64(),
                rate);

            // reset counters
            cur_time = SystemTime::now();
            cur_bytes = 0;
        }
    }

    Ok(())
}

// 4MiB chunk size
const CHUNK_SIZE: usize = 4 * 1024 * 1024;

/**
 * Uploader that generates an infinte strem of data and POSTs it to url
 * printing stats about the transfer to the console
 * @param url  Url to upload to
 */
pub async fn upload(url: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("[webclient] Uploading to {}", url);

    // async stream generating an infinite amount chunks
    let async_stream = async_stream::stream! {
        let mut cur_time = SystemTime::now();
        let mut cur_bytes: u64 = 0;
        loop {
            // every ~5 seconds print status
            let elapsed = cur_time.elapsed().unwrap();
            if elapsed >= Duration::from_secs(5) {
                // rate in mbit/s
                let rate = ((((cur_bytes as f64) * 8.0) / 1000.0) / 1000.0) / elapsed.as_secs_f64();
                println!("[webclient] Uploaded {} MB in {}s at rate {} Mbit/s",
                    cur_bytes / 1000 / 1000,
                    elapsed.as_secs_f64(),
                    rate);

                // reset counters
                cur_time = SystemTime::now();
                cur_bytes = 0;
            }
            let chunk = [255u8; CHUNK_SIZE];
            cur_bytes += chunk.len() as u64;
            yield Ok::<Vec<u8>, String>(chunk.to_vec());
        }
    };

    let _ = reqwest::Client::new()
        .post(url)
        .body(reqwest::Body::wrap_stream(async_stream))
        .send()
        .await
        .unwrap();

    Ok(())
}
