use csv::Reader;
use std::fs::File;
use std::str::FromStr;

use crate::rtnetlink_utils::get_interface_id_by_name;
use crate::rtnetlink_utils::qdisc_netem;
use crate::rtnetlink_utils::get_distribution;


struct TraceEvent {
    timestamp: f32,
    loss: u32
}

impl TraceEvent {
    pub fn new(timestamp: f32, loss: u32) -> Self {
        Self { timestamp, loss }
    }
}

struct Trace {
    trace: Vec<TraceEvent>
}

impl Trace {
    pub fn new(rdr: &mut csv::Reader<File>) -> Result<Self, String> {
        // indices from csv line
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
                trace.push(TraceEvent::new(timestamp, 100));
                // bridge end
                trace.push(TraceEvent::new(timestamp + time_under_bridge, 0));
            }
        }

        Ok(Self { trace })
    }

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
            .expect("Failed to get distribution data");

        // get interface ids
        let if_id = get_interface_id_by_name(handle.clone(), interface.clone())
            .await.unwrap();

        // initial state
        qdisc_netem(
            handle.clone(),
            if_id,
            false,
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
                true,
                1000,
                event.loss,
                37_500_000, // 300 mbit/s
                50_000_000, // 50 ms
                10_000_000,  // 10 ms
                distribution.clone()
            ).await.unwrap();
        }
        
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
