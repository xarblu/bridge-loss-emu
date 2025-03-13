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
     * timestamp,timeUnderBridge,speed
     */
    pub fn new(rdr: &mut csv::Reader<File>) -> Result<Self, String> {
        // CSV fields
        const CSV_IDX_TIMESTAMP: usize = 0;
        const CSV_IDX_TIME_UNDER_BRIDGE: usize = 1;
        const CSV_IDX_SPEED: usize = 2;

        let mut trace: Vec<TraceEvent> = Vec::new();
        
        let mut iter = rdr.records();
        let mut line = 1; // start at 1 due to header
        while let Some(result) = iter.next() {
            line += 1;
            let record = result.unwrap();
            let timestamp =
                f32::from_str(&record[CSV_IDX_TIMESTAMP])
                .map_err(|_| format!("Could not parse f32 from: {} on line {}",
                        String::from(&record[CSV_IDX_TIMESTAMP]), line))?;
            let time_under_bridge =
                f32::from_str(&record[CSV_IDX_TIME_UNDER_BRIDGE])
                .map_err(|_| format!("Could not parse f32 from: {} on line {}",
                        String::from(&record[CSV_IDX_TIME_UNDER_BRIDGE]), line))?;
            let _speed =
                f32::from_str(&record[CSV_IDX_SPEED])
                .map_err(|_| format!("Could not parse f32 from: {} on line {}",
                        String::from(&record[CSV_IDX_SPEED]), line))?;
            
            if time_under_bridge > 0.0f32 {
                // bridge start
                trace.push(TraceEvent::new(
                        timestamp,
                        100, // 100% bridge loss
                        50_000_000, // 50 ms
                        10_000_000 // 50 ms
                ));
                // bridge end
                trace.push(TraceEvent::new(
                        timestamp + time_under_bridge,
                        2, // 2% base loss
                        50_000_000, // 35 ms
                        10_000_000 // 10 ms
                ));
            }
        }

        // reconfigurations every 15s at 12,27,42,57 second of each minute
        // TODO: sync with actual wall clock time
        let mut reconf_timestamp = 12.0f32;
        let max_timestamp = trace.last().unwrap().timestamp;
        let mut trace_idx = 0;
        while reconf_timestamp < max_timestamp {
            // advance index until we reached next timestamp
            while trace[trace_idx].timestamp < reconf_timestamp {
                trace_idx += 1;
            }

            // get previous event to compare against and later
            // reset to
            // if there was no prior event assume all values to be 0
            // which will later always pick the bigger reconf params
            let mut prev = if trace_idx == 0 {
                TraceEvent::new(0.0,0,0,0)
            } else {
                trace[trace_idx - 1].clone()
            };

            // how long a reconfiguration will take
            // this is just an arbitrary 0.1s for now
            let reconf_duration = 0.1;

            // parameters for reconf
            let reconf_loss = 50; // 50% loss
            let reconf_latency = 50_000_000; // 50 ms
            let reconf_jitter = 10_000_000; // 10 ms

            // reconf start
            // pick the worst for each param
            // between what reconf would do and what already is present
            // e.g. a bridge causing 100% loss will always take
            // precedence over 50% reconf loss
            trace.insert(trace_idx,
                TraceEvent::new(
                    reconf_timestamp,
                    std::cmp::max(prev.loss, reconf_loss),
                    std::cmp::max(prev.latency, reconf_latency),
                    std::cmp::max(prev.jitter, reconf_jitter)
                )
            );
            trace_idx += 1;

            // figure out if we overlap with following events
            let mut overlap_idx = trace_idx;
            while trace[overlap_idx].timestamp < reconf_timestamp + reconf_duration {
                // can't go further
                if overlap_idx == trace.len() - 1 {
                    break;
                }
                overlap_idx += 1;
            }

            // overlapping events get the max of
            // their params and our reconf event
            while trace_idx < overlap_idx {
                let cur_event = trace[trace_idx].clone();
                let new_event = TraceEvent::new(
                    cur_event.timestamp,
                    std::cmp::max(cur_event.loss, reconf_loss),
                    std::cmp::max(cur_event.latency, reconf_latency),
                    std::cmp::max(cur_event.jitter, reconf_jitter)
                );
                trace[trace_idx] = new_event;
                // this event becomes the new previous event for our reset
                prev = cur_event;
                trace_idx += 1;
            }

            // reconf end
            // this simply resets to the last known state
            // before/during reconf
            trace.insert(trace_idx,
                TraceEvent::new(
                    reconf_timestamp + reconf_duration,
                    prev.loss,
                    prev.latency,
                    prev.jitter
                )
            );
            trace_idx += 1;

            // finally advance to next reconf
            reconf_timestamp += 15.0;
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

        // initial state
        qdisc_netem(
            handle.clone(),
            if_id,
            false, // replace qdisc
            1000,
            0,
            37_500_000, // 300 mbit/s
            50_000_000, // 50 ms
            10_000_000,  // 10 ms
            distribution.clone()
        ).await.unwrap();

        let start = tokio::time::Instant::now();
        let mut iter = self.trace.iter();
        while let Some(event) = iter.next() {
            let timestamp = tokio::time::Duration::from_secs_f32(event.timestamp);
            let _ = tokio::time::sleep_until(start + timestamp).await;
            let _ = qdisc_netem(
                handle.clone(),
                if_id,
                true, // change qdisc
                1000,
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
 * Convenence function to create and run a trace
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
