use csv::Reader;
use std::fs::File;
use tokio::runtime::Runtime;
use std::path::Path;
use fork::{fork, Fork};
use std::process::exit;

use crate::trace;
use crate::webserver;
use crate::testbed;
use crate::downloader;

pub fn run_test(rdr: &mut Reader<File>, file: String) {
    if !Path::new(file.as_str()).is_file() {
        eprintln!("File {} is not a regular file", file);
    }

    // setup testbed
    let testbed = testbed::Testbed::new();

    // start web server in a child process in namespace 1
    match fork() {
        Ok(Fork::Child) => {
            let _ = testbed.ns1.run(|_| {
                Runtime::new().unwrap().block_on(webserver::host_file(file));
            });

            exit(0); // just assume it was a success
        }
        Ok(Fork::Parent(child)) => println!("Spawned webserver process with pid: {}", child),
        Err(_) => eprintln!("Spawning webserver failed!")
    }

    // start a download in a child process in namespace 2
    match fork() {
        Ok(Fork::Child) => {
            let _ = testbed.ns2.run(|_| {
                let file_name = Path::new(file.as_str())
                    .file_name().unwrap();
                let url = String::from(
                    format!("http://{}:{}/{}",
                        testbed.addr1.as_str().split("/").next().unwrap(),
                        "8000",
                        file_name.to_str().unwrap()
                        ));
                let _ = Runtime::new().unwrap().block_on(downloader::download(url.as_str()));
            });

            exit(0); // just assume it was a success
        }
        Ok(Fork::Parent(child)) => println!("Spawned downloader process with pid: {}", child),
        Err(_) => eprintln!("Spawning downloader failed!")
    }

    // start playback of the trace
    trace::run_trace(rdr, &testbed);

    // destroy the testbed
    testbed.destroy();
}
