use csv::Reader;
use std::fs::File;
use std::process::exit;

use crate::rtnetlink_utils::{get_interface_id_by_name,qdisc_fq_codel};
use crate::trace;

/**
 * Cleanup funktion that:
 * - resets qdisc to fq_codel
 *
 * TODO:
 * - reset to actual prior qdisc
 *
 * @param interface  Interface name to reset qdisc on
 */
async fn cleanup(interface: String) {
    let (connection, handle, _) = rtnetlink::new_connection().unwrap();
    tokio::spawn(connection);
    let interface_id = get_interface_id_by_name(
        handle.clone(), interface.clone())
        .await.unwrap();

    let _ = qdisc_fq_codel(handle, interface_id)
        .await.unwrap();
}

/**
 * Run this test module
 * @param rdr        CSV reader of the trace file
 * @param unterface  Interface name used for trace playback
 */
pub fn run_test(
    rdr: &mut Reader<File>,
    distribution_file: Option<String>,
    interface: String
) {
    // shutdown handler
    let iface = interface.clone();
    ctrlc::set_handler(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(cleanup(iface.clone()));
        exit(1);
    }).expect("Error setting Ctrl-C handler");

    // start playback of the trace
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(trace::run_trace(rdr, distribution_file.clone(),
        interface.clone(), None));

    // cleanup when trace is done
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(cleanup(interface.clone()));
}
