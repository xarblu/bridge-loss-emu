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

        let mut trace: Vec<TraceEvent> = Vec::new();

        // initial state
        trace.push(TraceEvent::new(
                0.0,
                2, // 2% base loss
                27_600_000, // 27.6 ms GER-Highway
                10_000_000 // 10 ms jitter
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
                    27_600_000, // 27.6 ms GER-Highway
                    10_000_000 // 10 ms jitter
            ));
            // loss end
            trace.push(TraceEvent::new(
                    timestamp + loss_time,
                    2, // 2% loss
                    27_600_000, // 27.6 ms GER-Highway
                    10_000_000 // 10 ms jitter
            ));
        }

        Ok(Self { trace })
    }

    /**
     * Run a Trace
     * @param distribution_file  Optional path to a distribution file
     *                           Defaults to /lib64/tc/pareto.dist
     * @param interface          Interface where trace should run
     */
    pub async fn run(
        &mut self,
        distribution_file: Option<String>,
        interface: String
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


        let start = tokio::time::Instant::now();
        let mut iter = self.trace.iter();

        // first event has to replace qdisc
        if let Some(event) = iter.next() {
            qdisc_netem(
                handle.clone(),
                if_id,
                false, // replace qdisc
                1000, // pkts in queue
                event.loss,
                37_500_000, // 300 mbit/s
                event.latency,
                event.jitter,
                distribution.clone()
            ).await.unwrap();
        }

        // other events
        while let Some(event) = iter.next() {
            let timestamp = tokio::time::Duration::from_secs_f32(event.timestamp);
            let _ = tokio::time::sleep_until(start + timestamp).await;
            let _ = qdisc_netem(
                handle.clone(),
                if_id,
                true, // change qdisc
                1000, // pkts in queue
                event.loss,
                37_500_000, // 300 mbit/s
                event.latency,
                event.jitter,
                distribution.clone()
            ).await.unwrap();
        }
        
        println!("[trace] Reached end of trace");

        Ok(())
    }
}

/**
 * Convenience function to create and run a trace
 * @param rdr        csv::Reader for the trace
 * @param interface  Name of the interface the trace should run on
 */ 
pub async fn run_trace(
    rdr: &mut Reader<File>,
    distribution_file: Option<String>,
    interface: String
) {
    let mut trace = Trace::new(rdr).unwrap();
    let _ = trace.run(distribution_file, interface).await;
}
