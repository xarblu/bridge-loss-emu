use csv::Reader;
use std::fs::File;
use fork::{fork, Fork};
use std::process::exit;
use nix::unistd::Pid;
use nix::sys::signal::{self, Signal};

use crate::testbed;
use crate::trace;
use crate::webclient;
use crate::webserver;

pub fn run_test(
    rdr: &mut Reader<File>,
    distribution_file: Option<String>
) {
    // setup testbed
    let testbed = testbed::Testbed::new();

    // start web server in a child process in namespace 1
    let mut pid_server = -1;
    match fork() {
        Ok(Fork::Child) => {
            let _ = testbed.ns1.run(|_| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(webserver::rocket_main());
            });

            exit(0); // just assume it was a success
        }
        Ok(Fork::Parent(child)) => {
            println!("Spawned webserver process with pid: {}", child);
            pid_server = child;
        },
        Err(_) => eprintln!("Spawning webserver failed!")
    }

    // start a download in a child process in namespace 2
    let mut pid_client = -1;
    match fork() {
        Ok(Fork::Child) => {
            let _ = testbed.ns2.run(|_| {
                let url = format!("http://{}:{}/{}",
                        testbed.addr1.as_str().split("/").next().unwrap(),
                        "8000",
                        "infinite-data"
                        );
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(webclient::download(url.clone())).unwrap();
            });

            exit(0); // just assume it was a success
        }
        Ok(Fork::Parent(child)) => {
            println!("Spawned webclient process with pid: {}", child);
            pid_client = child;
        },
        Err(_) => eprintln!("Spawning webclient failed!")
    }

    // start playback of the trace
    let _ = testbed.ns2.run(|_| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(trace::run_trace(rdr, distribution_file.clone(), testbed.if2.clone()));
    });

    // cleanup when trace is done
    println!("Reached end of trace - shutting down");
    signal::kill(Pid::from_raw(pid_server), Signal::SIGTERM).unwrap();
    signal::kill(Pid::from_raw(pid_client), Signal::SIGTERM).unwrap();

    // destroy the testbed
    testbed.destroy();
}
