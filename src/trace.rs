use csv::Reader;
use std::fs::File;
use std::str::FromStr;

use crate::rtnetlink_utils::get_interface_id_by_name;
use crate::rtnetlink_utils::qdisc_netem;
use crate::rtnetlink_utils::get_distribution;


#[derive(Clone)]
struct TraceEvent {
    timestamp: f32,
    loss: u32,
    latency: i64,
    jitter: i64
}

impl TraceEvent {
    /**
     * Create a new TraceEvent
     * @param timestamp  Reative timestamp on Trace
     * @param loss       Loss in % (0-100)
     * @param latency    Added latency in ns
     * @param jitter     Jitter on latency in ns
     */
    pub fn new(timestamp: f32, loss: u32, latency: i64, jitter: i64) -> Self {
        Self { timestamp, loss, latency, jitter }
    }
}

struct Trace {
    trace: Vec<TraceEvent>
}

impl Trace {
    /**
     * Create a new Trace from CSV file Reader
     * @param rdr  CSV file reader
     *
     * Currently expects format:
     * timestamp,lossTime
     */
    pub fn new(rdr: &mut csv::Reader<File>) -> Result<Self, String> {
        // CSV fields
        const CSV_IDX_TIMESTAMP: usize = 0;
        const CSV_IDX_LOSS_TIME: usize = 1;

        // base loss for "clean" traffic
        // paper says this is ~2% but that destroys download/upload tests
        // because the TCP congestion control keeps decreasing the bandwidth
        const BASE_LOSS: u32 = 0;

        // currently unchanging latency and jitter
        // during playback these will be doubled
        // because they apply to both the egress (if) and ingress (ifb)
        // taken from
        // https://github.com/sys-uos/Starlink-on-the-Autobahn/blob/main/loss_emulation.py
        const LATENCY: i64 = 18_000_000; // total 36 ms
        const JITTER: i64 = 16_500_000; // total 33 ms

        // trace vector
        let mut trace: Vec<TraceEvent> = Vec::new();

        // initial state
        trace.push(TraceEvent::new(
                0.0,
                BASE_LOSS,
                LATENCY,
                JITTER
        ));
        
        let mut iter = rdr.records();
        let mut line = 1; // start at 1 due to header
        while let Some(result) = iter.next() {
            line += 1;
            let record = result.unwrap();
            let timestamp =
                f32::from_str(&record[CSV_IDX_TIMESTAMP])
                .map_err(|_| format!("Could not parse f32 from: {} on line {}",
                        String::from(&record[CSV_IDX_TIMESTAMP]), line))?;
            let loss_time =
                f32::from_str(&record[CSV_IDX_LOSS_TIME])
                .map_err(|_| format!("Could not parse f32 from: {} on line {}",
                        String::from(&record[CSV_IDX_LOSS_TIME]), line))?;

            // loss start
            trace.push(TraceEvent::new(
                    timestamp,
                    100, // 100% loss
                    LATENCY,
                    JITTER
            ));
            // loss end
            trace.push(TraceEvent::new(
                    timestamp + loss_time,
                    BASE_LOSS,
                    LATENCY,
                    JITTER
            ));
        }

        Ok(Self { trace })
    }

    /**
     * Run a Trace
     * @param distribution_file  Optional path to a distribution file
     *                           Defaults to /lib64/tc/pareto.dist
     * @param interface          Interface where trace should run
     * @param ifb                Intermediate Function Block attached to interface
     */
    pub async fn run(
        &mut self,
        distribution_file: Option<String>,
        interface: String,
        ifb: Option<String>
    ) -> Result<(), String> {
        // setup handle and connection for rtnetlink stuff
        let (connection, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(connection);

        let distribution = get_distribution(
            distribution_file.unwrap_or(String::from("/lib64/tc/pareto.dist")))
            .await
            .expect("[trace] Failed to get distribution data");

        // get interface ids
        let if_id = get_interface_id_by_name(handle.clone(), interface.clone())
            .await.unwrap();

        let mut ifb_id: Option<u32> = None;
        if let Some(ifb) = ifb {
            ifb_id = Some(get_interface_id_by_name(handle.clone(), ifb.clone())
                .await.unwrap());
        }

        // constants for settings that don't change
        // taken from
        // https://github.com/sys-uos/Starlink-on-the-Autobahn/blob/main/loss_emulation.py
        const LIMIT: u32 = 10_000; // pkts in queue
        const RATE: u64 = 37_500_000; // 300 mbit/s

        let start = tokio::time::Instant::now();
        let mut iter = self.trace.iter();

        // first event has to replace qdisc
        if let Some(event) = iter.next() {
            // outgoing traffic
            qdisc_netem(
                handle.clone(),
                if_id,
                false, // replace qdisc
                LIMIT,
                event.loss,
                RATE,
                event.latency,
                event.jitter,
                distribution.clone()
            ).await.unwrap();

            // incoming traffic
            if let Some(ifb_id) = ifb_id {
                qdisc_netem(
                    handle.clone(),
                    ifb_id,
                    false, // replace qdisc
                    LIMIT,
                    event.loss,
                    RATE,
                    event.latency,
                    event.jitter,
                    distribution.clone()
                ).await.unwrap();
            }
        }

        // other events
        while let Some(event) = iter.next() {
            let timestamp = tokio::time::Duration::from_secs_f32(event.timestamp);
            let _ = tokio::time::sleep_until(start + timestamp).await;
            // outgoing traffic
            let _ = qdisc_netem(
                handle.clone(),
                if_id,
                true, // change qdisc
                LIMIT,
                event.loss,
                RATE,
                event.latency,
                event.jitter,
                distribution.clone()
            ).await.unwrap();

            // incoming traffic
            if let Some(ifb_id) = ifb_id {
                qdisc_netem(
                    handle.clone(),
                    ifb_id,
                    true, // change qdisc
                    LIMIT,
                    event.loss,
                    RATE,
                    event.latency,
                    event.jitter,
                    distribution.clone()
                ).await.unwrap();
            }
        }
        
        println!("[trace] Reached end of trace");

        Ok(())
    }
}

/**
 * Convenience function to create and run a trace
 * @param rdr        csv::Reader for the trace
 * @param interface  Name of the interface the trace should run on
 * @param ifb        Intermediate Function Block attached to interface
 */ 
pub async fn run_trace(
    rdr: &mut Reader<File>,
    distribution_file: Option<String>,
    interface: String,
    ifb: Option<String>
) {
    let mut trace = Trace::new(rdr).unwrap();
    let _ = trace.run(distribution_file, interface, ifb).await;
}
