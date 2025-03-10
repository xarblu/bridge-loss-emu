use futures::StreamExt;
use std::time::{SystemTime, Duration};

pub async fn download(url: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading from {}", url);

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap();

    let mut stream = response.bytes_stream();

    let mut cur_time = SystemTime::now();
    let total_time = cur_time.clone();
    let mut cur_bytes: u64 = 0;
    let mut total_bytes: u64 = 0;
    while let Some(item) = stream.next().await {
        let chunk = item.expect("Download failed");
        cur_bytes += chunk.len() as u64;

        // every ~5 seconds print status
        let elapsed = cur_time.elapsed().unwrap();
        if elapsed >= Duration::from_secs(5) {
            // rate in mbit/s
            let rate = ((((cur_bytes as f64) * 8.0) / 1024.0) / 1024.0) / elapsed.as_secs_f64();
            println!("Downloaded {} MB in {} s at rate {} Mbit/s",
                cur_bytes / 1024 / 1024,
                elapsed.as_secs_f64(),
                rate);

            // increase total counter
            total_bytes += cur_bytes;

            // reset counters
            cur_time = SystemTime::now();
            cur_bytes = 0;
        }
    }

    // total progress
    let elapsed = total_time.elapsed().unwrap();
    let rate = ((((cur_bytes as f64) * 8.0) / 1024.0) / 1024.0) / elapsed.as_secs_f64();
    println!("Downloaded {} MB in {} s at rate {} Mbit/s total",
        total_bytes / 1024 / 1024,
        elapsed.as_secs_f64(),
        rate);

    Ok(())
}

pub async fn upload(url: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("Uploading to {}", url);

    // async stream generating an infinite amount of 4KiB chunks
    let async_stream = async_stream::stream! {
        let mut cur_time = SystemTime::now();
        let mut cur_bytes: u64 = 0;
        loop {
            // every ~5 seconds print status
            let elapsed = cur_time.elapsed().unwrap();
            if elapsed >= Duration::from_secs(5) {
                // rate in mbit/s
                let rate = ((((cur_bytes as f64) * 8.0) / 1024.0) / 1024.0) / elapsed.as_secs_f64();
                println!("Uploaded {} MB in {} s at rate {} Mbit/s",
                    cur_bytes / 1024 / 1024,
                    elapsed.as_secs_f64(),
                    rate);

                // reset counters
                cur_time = SystemTime::now();
                cur_bytes = 0;
            }
            let bytes: Vec<u8> = vec![255u8; 4096];
            cur_bytes += 4096;
            yield Ok::<Vec<u8>, String>(bytes);
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
