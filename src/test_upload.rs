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
    distribution_file: Option<String>,
    capture_file: Option<String>,
) {
    // setup testbed
    let testbed = testbed::Testbed::new();

    // start web server in a child process in namespace 1
    let mut pid_server = -1;
    match fork() {
        Ok(Fork::Child) => {
            let _ = testbed.ns1.run(|_| {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .thread_name("webserver")
                    .enable_all()
                    // increased stack size for big chunks
                    .thread_stack_size(100 * 1024 * 1024)
                    .build()
                    .expect("[test] failed to create tokio runtime");
                rt.block_on(webserver::rocket_main());
            });

            exit(0); // just assume it was a success
        }
        Ok(Fork::Parent(child)) => {
            println!("[test] Spawned webserver process with pid: {}", child);
            pid_server = child;
        },
        Err(_) => eprintln!("[test] Spawning webserver failed!")
    }

    // start a upload in a child process in namespace 2
    let mut pid_client = -1;
    match fork() {
        Ok(Fork::Child) => {
            let _ = testbed.ns2.run(|_| {
                let url = format!("http://{}:{}/{}",
                        testbed.addr1.as_str().split("/").next().unwrap(),
                        "8000",
                        "infinite-data"
                        );
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .thread_name("webclient")
                    .enable_all()
                    // increased stack size for big chunks
                    .thread_stack_size(100 * 1024 * 1024)
                    .build()
                    .expect("[test] failed to create tokio runtime");
                rt.block_on(webclient::upload(url.clone())).unwrap();
            });

            exit(0); // just assume it was a success
        }
        Ok(Fork::Parent(child)) => {
            println!("[test] Spawned webclient process with pid: {}", child);
            pid_client = child;
        },
        Err(_) => eprintln!("[test] Spawning webclient failed!")
    }

    // start tshark in namespace 2
    let mut pid_tshark = -1;
    if let Some(capture_file) = capture_file {
        match fork() {
            Ok(Fork::Child) => {
                let _ = testbed.ns2.run(|_| {
                    let _ = std::process::Command::new("tshark")
                        .args([
                            "-w", capture_file.as_str(),
                            "-i", &testbed.if2
                        ])
                        //.stdout(Stdio::null())
                        .status()
                        .expect("[test] Spawning mpv process failed");
                });

                exit(0); // just assume it was a success
            }
            Ok(Fork::Parent(child)) => {
                println!("[test] Spawned mpv process with pid: {}", child);
                pid_tshark = child;
            },
            Err(_) => eprintln!("[test] Spawning mpv failed!")
        }
    }

    // start playback of the trace
    let _ = testbed.ns2.run(|_| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(trace::run_trace(rdr, distribution_file.clone(),
            testbed.if2.clone(), Some(testbed.ifb2.clone())));
    });

    // cleanup when trace is done
    signal::kill(Pid::from_raw(pid_server), Signal::SIGTERM).unwrap();
    signal::kill(Pid::from_raw(pid_client), Signal::SIGTERM).unwrap();
    if pid_tshark > 0 {
        signal::kill(Pid::from_raw(pid_tshark), Signal::SIGTERM).unwrap();
    }

    // destroy the testbed
    testbed.destroy();
}
