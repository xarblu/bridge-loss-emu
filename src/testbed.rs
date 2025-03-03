use netns_rs::NetNs;
use std::process::{Command, Stdio};

pub struct Testbed {
    pub ns1: NetNs,
    pub ns2: NetNs,
    pub if1: String,
    pub if2: String,
    pub addr1: String,
    pub addr2: String,
}

impl Testbed {
    pub fn new() -> Testbed {
        // delete namespaces if they exist
        for name in ["ns1", "ns2"] {
            let ns = NetNs::get(name);
            if ns.is_ok() {
                println!("{} exists - recreating", name);
                let _ = ns.unwrap().remove();
            }
        }

        // create new testbed
        let new = Self {
            ns1: NetNs::new("ns1").expect("Creating ns1 failed"),
            ns2: NetNs::new("ns2").expect("Createing ns2 failed"),
            if1: String::from("veth1"),
            if2: String::from("veth2"),
            addr1: String::from("10.0.0.1/24"),
            addr2: String::from("10.0.0.2/24"),
        };
        
        // delete interfaces if they existm then create bew ones
        for if_name in [new.if1.as_str(), new.if2.as_str()] {
            let if_status = Command::new("ip")
                .args(["link", "show", "dev", if_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            
            if if_status.unwrap().success() {
                println!("Removing existing interface {}", if_name);
                let _ = Command::new("ip")
                    .args(["link", "delete", "dev", if_name])
                    .status();
            }
        }

        println!("Creating new interfaces");
        let _ = Command::new("ip")
            .args([
                "link", "add", "dev", new.if1.as_str(), "type", "veth",
                "peer", "name", new.if2.as_str()
            ])
            .status();

        // attach interfaces to namespaces
        for if_ns in [
            (new.if1.as_str(),&new.ns1),
            (new.if2.as_str(),&new.ns2) ] {
            let if_name = if_ns.0;
            let if_ns = &if_ns.1.path().file_name().unwrap().to_str().unwrap();

            let _ = Command::new("ip")
                .args(["link", "set", "dev", if_name, "netns", if_ns])
                .status()
                .expect(format!("Failed attaching {} to netns {}",
                        if_name, if_ns).as_str());
        }
        
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

        // return Testbaed
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
