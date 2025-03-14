use netns_rs::NetNs;
use std::process::{Command, Stdio};

pub struct Testbed {
    pub ns1: NetNs,
    pub ns2: NetNs,
    pub if1: String,
    pub if2: String,
    pub ifb2: String,
    pub addr1: String,
    pub addr2: String,
}

impl Testbed {
    pub fn new() -> Testbed {
        // delete namespaces if they exist
        for name in ["ns1", "ns2"] {
            let ns = NetNs::get(name);
            if ns.is_ok() {
                println!("[testbed] {} exists - recreating", name);
                let _ = ns.unwrap().remove();
            }
        }

        // create new testbed
        let new = Self {
            ns1: NetNs::new("ns1").expect("Creating ns1 failed"),
            ns2: NetNs::new("ns2").expect("Createing ns2 failed"),
            if1: String::from("veth1"),
            if2: String::from("veth2"),
            ifb2: String::from("ifb2"),
            addr1: String::from("10.0.0.1/24"),
            addr2: String::from("10.0.0.2/24"),
        };
        
        // delete interfaces if they exist then create new ones
        for if_name in [new.if1.as_str(), new.if2.as_str(), new.ifb2.as_str()] {
            let if_status = Command::new("ip")
                .args(["link", "show", "dev", if_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            
            if if_status.unwrap().success() {
                println!("[testbed] Removing existing interface {}", if_name);
                let _ = Command::new("ip")
                    .args(["link", "delete", "dev", if_name])
                    .status();
            }
        }

        println!("[testbed] Creating new interfaces in network namespaces");
        let _ = Command::new("ip")
            .args([
                "link", "add", "dev", new.if1.as_str(),
                "netns", &new.ns1.path().file_name().unwrap().to_str().unwrap(),
                "type", "veth",
                "peer", "name", new.if2.as_str(),
                "netns", &new.ns2.path().file_name().unwrap().to_str().unwrap(),
            ])
            .status();

        // finally set interfaces UP
        for spec in [
            &(new.if1.as_str(), &new.ns1, new.addr1.as_str()), 
            &(new.if2.as_str(), &new.ns2, new.addr2.as_str())
        ] {
            let if_name = &spec.0;
            let ns = &spec.1;
            let addr = &spec.2;
            let _ = ns.run(|_| {
                // setup address
                let _ = Command::new("ip")
                    .args(["addr", "add", addr,
                        "dev", if_name])
                    .status();

                // set UP
                let _ = Command::new("ip")
                    .args(["link", "set", "dev", if_name, "up"])
                    .status();
            });
        }

        // since qdiscs only affect outgoing traffic we need this bridge device
        // to add netem to incoming traffic
        println!("[testbed] Setting up ifb interface {} to handle incoming traffic for {}",
            new.ifb2.as_str(), new.if2.as_str());
        let _ = Command::new("ip").args([
                "link", "add", "name", new.ifb2.as_str(),
                "netns", &new.ns2.path().file_name().unwrap().to_str().unwrap(),
                "type", "ifb"
            ]).status();
        let _ = new.ns2.run(|_| {
            let _ = Command::new("ip").args([
                    "link", "set", "dev", new.ifb2.as_str(), "up"
                ]).status();
            // redirect incoming traffic through ifb2
            let _ = Command::new("tc").args([
                    "qdisc", "add", "dev", new.if2.as_str(), "ingress"
                ]).status();
            let _ = Command::new("tc").args([
                    "filter", "add", "dev", new.if2.as_str(), "parent", "ffff:",
                    "protocol", "ip", "u32", "match", "u32", "0", "0", "flowid", "1:1",
                    "action", "mirred", "egress", "redirect", "dev", new.ifb2.as_str()
                ]).status();
        });

        // return Testbed
        new
    }

    // destroy this Testbed (clean up namespaces and interfaces)
    pub fn destroy(&self) {
        for spec in [
            (self.if1.as_str(), &self.ns1, self.addr1.as_str()), 
            (self.if2.as_str(), &self.ns2, self.addr2.as_str())
        ] {
            let if_name = &spec.0;
            let ns = &spec.1;
            let addr = &spec.2;
            let _ = ns.run(|_| {
                // remove qdisc
                let _ = Command::new("tc")
                    .args(["qdisc", "delete", "dev", if_name, "root"])
                    .status();

                // set interface DOWN
                let _ = Command::new("ip")
                    .args(["link", "set", "dev", if_name, "down"])
                    .status();

                // remove address
                let _ = Command::new("ip")
                    .args(["addr", "delete", addr,
                        "dev", if_name])
                    .status();
            });

            // remove interface
            let _ = Command::new("ip")
                .args(["link", "delete", if_name])
                .status();
            
            // remove namespace
            //ns.remove();
        }
    }
}
