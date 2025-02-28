use csv::Reader;
use std::fs::File;
use tokio::runtime::Runtime;
use std::path::Path;
use fork::{fork, Fork};
use std::process::exit;

use crate::trace;
use crate::webserver;
use crate::testbed;

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
        Ok(Fork::Parent(child)) => println!("Spawned child with pid: {}", child),
        Err(_) => eprintln!("Spawing webserver failed!")
    }

    trace::run_trace(rdr, &testbed);

    // destroy the testbed
    testbed.destroy();
}
