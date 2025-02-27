use netns_rs::NetNs;
use std::{process::{Command, ExitStatus, exit}, time::Duration};
use csv::Reader;
use clap::Parser;
use std::str::FromStr;
use users::get_effective_uid;

mod testbed;

use testbed::Testbed;

const CSV_IDX_REL_TIME: usize = 0;
const CSV_IDX_LOSS: usize = 1;

/// Emulator for packet loss caused by bridges
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// CSV file with loss trace of form relative_time,loss
    #[arg(short, long)]
    file: String,

    /// Test to run
    #[arg(short, long, default_value_t = String::from("download"))]
    test: String,
}

enum Test {
    Download,
    Upload,
    Stream
}

fn cmd_in_net_ns(ns: &NetNs, cmd: &[&str]) -> ExitStatus {
    ns.run(|_| {
        Command::new(&cmd[0])
            .args(&cmd[1..])
            .status()
            .expect("Failed running command in network namespace")
    }).unwrap()
}

fn init_default_state(ns: &NetNs, interface: &str) {
    let status = cmd_in_net_ns(ns, 
        &["tc", "qdisc", "add", "dev", interface, "root", "netem",
        "rate", "300mbit",
        "loss", "0%",
        "delay", "36ms", "33ms",
        "distribution", "pareto",
        "seed", "42",
        "limit", "100000"]);
    println!("Initial loss set to 0% ({status})");
}

fn set_default_state(ns: &NetNs, interface: &str) {
    let status = cmd_in_net_ns(ns, 
        &["tc", "qdisc", "replace", "dev", interface, "root", "netem",
        "rate", "300mbit",
        "loss", "0%",
        "delay", "36ms", "33ms",
        "distribution", "pareto",
        "seed", "42",
        "limit", "100000"]);
    println!("Loss set to 0% ({status})");
}

fn set_bridge_state(ns: &NetNs, interface: &str) {
    let status = cmd_in_net_ns(ns, 
        &["tc", "qdisc", "replace", "dev", interface, "root", "netem",
        "rate", "300mbit",
        "loss", "100%",
        "delay", "36ms", "33ms",
        "distribution", "pareto",
        "seed", "42",
        "limit", "100000"]);
    println!("Loss set to 100% ({status})");
}

fn main() {
    // setup and checks
    let args = Args::parse();

    // we need to be root in order to create network namespaces or interfaces
    if get_effective_uid() != 0 {
        eprintln!("Elevated privileges are required \
            to create network namespaces or interfaces");
        exit(1);
    }

    // try to read file
    let mut rdr = Reader::from_path(args.file.as_str()).unwrap_or_else(|_| {
        eprintln!("Could not open csv file {} for reading", args.file.as_str());
        exit(1);
    });

    // match test
    let test = match args.test.as_str() {
        "download" => Test::Download,
        _ => {
            eprintln!("Unknown test {}", args.test.as_str());
            exit(1);
        }
    };

    println!("Setting up testbed...");
    let testbed = Testbed::new();

    // actual emulation code
    init_default_state(&testbed.ns2, "veth2");

    // cyncle through each entry in the csv and toggle netem accordingly
    let mut iter = rdr.records().peekable();
    let mut line = 1; // start at 1 due to header
    while iter.peek().is_some() {
        let result = iter.next().unwrap();
        line += 1;
        let record = result.unwrap();
        let relative_time = f32::from_str(&record[CSV_IDX_REL_TIME])
            .unwrap_or_else(|_| {
                eprintln!("Could not parse f32 from: {} on line {}",
                    String::from(&record[CSV_IDX_REL_TIME]), line);
                exit(1);
            });
        let lost = &record[CSV_IDX_LOSS];

        match lost.into() {
            "True" => set_bridge_state(&testbed.ns2, "veth2"),
            "False" => set_default_state(&testbed.ns2, "veth2"),
            _ => {
                eprintln!("Could not parse True/False from: {}", lost);
                exit(1);
            }
        };
        
        // check if there's another entry and wait until its timestamp
        if iter.peek().is_some() {
            let next_result = iter.peek().unwrap();
            let next_record = next_result.as_ref().unwrap();
            let next_relative_time = f32::from_str(&next_record[CSV_IDX_REL_TIME])
                .unwrap_or_else(|_| {
                    eprintln!("Could not parse f32 from: {} on line {}",
                        &next_record[CSV_IDX_REL_TIME], line + 1);
                    exit(1);
                });
            let time_to_wait = next_relative_time - relative_time;
            println!("{time_to_wait}s until next");
            std::thread::sleep(Duration::from_secs_f32(time_to_wait));
        }
    }

    // destroy the testbed
    testbed.destroy();
}
