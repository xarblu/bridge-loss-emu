use csv::Reader;
use std::fs::File;
use std::str::FromStr;
use std::process::exit;
use std::time::Duration;

use crate::rtnetlink_utils::get_interface_id_by_name;
use crate::rtnetlink_utils::replace_interface_qdisc_netem;
use crate::rtnetlink_utils::get_distribution;

/// relative_time entry in the trace csv
const CSV_IDX_REL_TIME: usize = 0;

/// loss entry in the trace csv
const CSV_IDX_LOSS: usize = 1;

/**
 * Start playback of the loss trace
 * @param rdr        csv::Reader for the trace
 * @param interface  Name of the interface the trace should run on
 */ 
pub async fn run_trace(rdr: &mut Reader<File>, interface: String) {
    // setup handle and connection for rtnetlink stuff
    let (connection, handle, _) = rtnetlink::new_connection().unwrap();
    tokio::spawn(connection);

    let distribution = get_distribution(String::from("/lib64/tc/pareto.dist"))
        .await
        .expect("Failed to get distribution data");

    // get interface ids
    let if_id = get_interface_id_by_name(handle.clone(), interface.clone())
        .await.unwrap();

    // initial state
    replace_interface_qdisc_netem(
        handle.clone(),
        if_id,
        1000,
        0,
        37_500_000, // 300 mbit/s
        50_000_000, // 50 ms
        10_000_000,  // 10 ms
        distribution.clone()
    ).await.unwrap();

    // cycle through each entry in the csv and toggle netem accordingly
    let mut iter = rdr.records().peekable();
    let mut line = 1; // start at 1 due to header
    while let Some(result) = iter.next() {
        let record = result.unwrap();
        line += 1;
        let relative_time = f32::from_str(&record[CSV_IDX_REL_TIME])
            .unwrap_or_else(|_| {
                eprintln!("Could not parse f32 from: {} on line {}",
                    String::from(&record[CSV_IDX_REL_TIME]), line);
                exit(1);
            });
        let lost = &record[CSV_IDX_LOSS];

        let _ = match lost.into() {
            "True" => replace_interface_qdisc_netem(
                        handle.clone(),
                        if_id,
                        1000,
                        100,
                        37_500_000, // 300 mbit/s
                        50_000_000, // 50 ms
                        10_000_000,  // 10 ms
                        distribution.clone()
                    ).await,
            "False" => replace_interface_qdisc_netem(
                        handle.clone(),
                        if_id,
                        1000,
                        0,
                        37_500_000, // 300 mbit/s
                        50_000_000, // 50 ms
                        10_000_000,  // 10 ms
                        distribution.clone()
                    ).await,
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
}
