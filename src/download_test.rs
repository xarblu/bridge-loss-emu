use csv::Reader;
use std::fs::File;
use tokio::runtime::Runtime;
use std::path::Path;
use fork::{fork, Fork};

use crate::trace;
use crate::webserver;

pub fn run_test(rdr: &mut Reader<File>, file: String) {
    if !Path::new(file.as_str()).is_file() {
        eprintln!("File {} is not a regular file", file);
    }

    // start web server in a child process
    match fork() {
        Ok(Fork::Child) => Runtime::new().unwrap().block_on(webserver::host_file(file)),
        Ok(Fork::Parent(child)) => println!("Spawned child with pid: {}", child),
        Err(_) => eprintln!("Spawing webserver failed!")
    }

    trace::run_trace(rdr);
}
