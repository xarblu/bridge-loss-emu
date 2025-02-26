use netns_rs::NetNs;
use std::{process::{Command, ExitStatus}, time::Duration};
use csv::Reader;
use chrono::DateTime;

fn cmd_in_net_ns(ns: &NetNs, cmd: &[&str]) -> ExitStatus {
    ns.run(|_| {
        Command::new(&cmd[0])
            .args(&cmd[1..])
            .status()
            .expect("Failed running command in network namespace")
    }).unwrap()
}

fn del_qdisc(ns: &NetNs, interface: &str) {
    let status = cmd_in_net_ns(ns, 
        &["tc", "qdisc", "delete", "dev", interface, "root"]);
    println!("QDisc removed ({status})");
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
    // init 2 network namespaces
    let ns1 = NetNs::new("ns1")
        .unwrap_or_else(|_| {
            println!("ns1 already exists - reusing");
            NetNs::get("ns1").unwrap()
        });
    let ns2 = NetNs::new("ns2")
        .unwrap_or_else(|_| {
            println!("ns2 already exists - reusing");
            NetNs::get("ns2").unwrap()
        });
    
    // inside each ns create a virtual interface
    println!("Creating virtual interfaces");
    Command::new("ip")
        .args(["link", "add", "veth1", "type", "veth", "peer", "name", "veth2"])
        .spawn()
        .expect("Failed interface creation");

    println!("Attaching interfaces to network namespaces");
    Command::new("ip")
        .args(["link", "set", "veth1", "netns", "ns1"])
        .spawn()
        .expect("Failed attaching veth1 to ns1");
    Command::new("ip")
        .args(["link", "set", "veth2", "netns", "ns2"])
        .spawn()
        .expect("Failed attaching veth2 to ns2");

    println!("Setting up addresses");
    cmd_in_net_ns(&ns1, &["ip", "addr", "add", "10.0.0.1/24", "dev", "veth1"]);
    cmd_in_net_ns(&ns2, &["ip", "addr", "add", "10.0.0.2/24", "dev", "veth2"]);
    cmd_in_net_ns(&ns1, &["ip", "link", "set", "veth1", "up"]);
    cmd_in_net_ns(&ns2, &["ip", "link", "set", "veth2", "up"]);

    
    // actual emulation code
    init_default_state(&ns2, "veth2");

    let mut rdr = Reader::from_path("Emulation/loss_time_bridge_trace.csv").unwrap();

    let mut iter = rdr.records().peekable();

    while iter.peek().is_some() {
        let result = iter.next().unwrap();
        let record = result.unwrap();
        let relative_time = record[0].parse::<f32>().unwrap();
        let lost = &record[1];

        match lost.into() {
            "True" => set_bridge_state(&ns2, "veth2"),
            "False" => set_default_state(&ns2, "veth2"),
            _ => panic!("Parse error: {lost}")
        };
        
        // check if there's another entry and wait until its timestamp
        if iter.peek().is_some() {
            let next_result = iter.peek().unwrap();
            let next_record = next_result.as_ref().unwrap();
            let next_relative_time = next_record[0].parse::<f32>().unwrap();
            let time_to_wait = next_relative_time - relative_time;
            println!("{time_to_wait}s until next");
            std::thread::sleep(Duration::from_secs_f32(time_to_wait));
        }
    }
    

    // cleanup
    println!("Cleanup");
    cmd_in_net_ns(&ns1, &["ip", "link", "set", "veth1", "down"]);
    cmd_in_net_ns(&ns2, &["ip", "link", "set", "veth2", "down"]);
    cmd_in_net_ns(&ns1, &["ip", "addr", "delete", "10.0.0.1/24", "dev", "veth1"]);
    cmd_in_net_ns(&ns2, &["ip", "addr", "delete", "10.0.0.2/24", "dev", "veth2"]);
    
    Command::new("ip")
        .args(["link", "delete", "veth1"])
        .spawn()
        .expect("Failed interface deletion");

    let _ = ns1.remove();
    let _ = ns2.remove();
}
