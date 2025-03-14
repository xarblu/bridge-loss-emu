use rtnetlink::Handle;
use netlink_packet_utils::{byteorder::{ByteOrder, NativeEndian}, nla::DefaultNla, traits::Emitable};
use netlink_packet_route::{link::LinkMessageBuffer, tc::{TcAttribute, TcOption}};
use futures::TryStreamExt;
use std::io::{self, BufRead};
use std::fs::File;
use std::str::FromStr;

/**
 * Get the internal id of an interface by its name
 * A working handle must be created in a tokio runtime like:
 * @param handle  Handle as obtained by rtnetlink::new_connection().
 *                Example:
 *                  let (connection, handle, _) = rtnetlink::new_connection().unwrap();
 *                  tokio::spawn(connection);
 * @param name    Name of the interface
 */
pub async fn get_interface_id_by_name(handle: Handle, name: String) -> Result<u32, String> {
    let mut stream = handle
        .link()
        .get()
        .match_name(name.clone())
        .execute();

    // first response is the requested index
    let response = stream.try_next().await;
    if response.is_err() {
        return Err(format!("RTNetlink error during lookup of interface {}",
                name.clone()));
    }
    // seems like this is never hit and non-present interface names
    // just throw an rtnetlink error 
    // but for completeness let's just keep this here
    if response.clone().is_ok_and(|x| x.is_none()) {
        return Err(format!("Could not find interface {}",
                name.clone()));
    }

    // convert LinkMessage to LinkMessageBuffer via intermediate buffer
    // there's probably a better way to do this but I couldn't find it
    let msg = response.unwrap().unwrap();
    let mut buf = vec![0; msg.buffer_len()];
    msg.emit(&mut buf);
    let msgbuf = LinkMessageBuffer::new(buf);
    let link_index = msgbuf.link_index();

    // the stream should now be empty, if not bail out
    let response = stream.try_next().await;
    if response.is_ok_and(|x| x.is_some()) {
        return Err(format!("Unexpected response during lookup of interface {}",
                name.clone()));
    }

    Ok(link_index)
}


/**
 * consts from /include/uapi/linux/pkt_sched.h
 * in the linux source tree
 * these map to the "kind" of nla message
 */

const TCA_NETEM_UNSPEC: u16 = 0;
const TCA_NETEM_CORR: u16 = 1;
const TCA_NETEM_DELAY_DIST: u16 = 2;
const TCA_NETEM_REORDER: u16 = 3;
const TCA_NETEM_CORRUPT: u16 = 4;
// const TCA_NETEM_LOSS: u16 = 5;
const TCA_NETEM_RATE: u16 = 6;
// const TCA_NETEM_ECN: u16 = 7;
const TCA_NETEM_RATE64: u16 = 8;
// const TCA_NETEM_PAD: u16 = 9;
const TCA_NETEM_LATENCY64: u16 = 10;
const TCA_NETEM_JITTER64: u16 = 11;
const TCA_NETEM_SLOT: u16 = 12;
// const TCA_NETEM_SLOT_DIST: u16 = 13;
// const TCA_NETEM_PRNG_SEED: u16 = 14;

/**
 * Common netem qdisc handler
 * @param handle        Handle for rtnetlink
 * @param interface_id  ID of the interface
 * @param inplace       Change qdiscc inplace
 *                      this requires handle to be the same as existing
 * @param limit         Limit for packets in queue
 * @param loss          Loss rate in percent (0-100)
 * @param rate          Rate limit in byte/s
 * @param latency       Added delay in ns
 * @param jitter        Jitter for delay in ns
 */
pub async fn qdisc_netem(
    handle: Handle,
    interface_id: u32,
    inplace: bool,
    limit: u32,
    loss: u32,
    rate: u64,
    latency: i64,
    jitter: i64,
    distribution: Vec<i16>
) -> Result<(), String> {
    // argument constraints
    if loss > 100 {
        return Err(format!("Loss value {} not in range [0..100]", loss));
    }

    let mut request = if inplace {
        handle
        .qdisc()
        .change(interface_id as i32)
        .root()
    } else {
        handle
        .qdisc()
        .replace(interface_id as i32)
        .root()

    };

    // add qdisc kind
    request.message_mut().attributes.push(
        TcAttribute::Kind(String::from("netem")));

    // add options
    let mut options: Vec<TcOption> = Vec::new();

    // these emulate the structs from
    // /include/uapi/linux/pkt_sched.h
    // may be x86 exclusive because of this but whatever
    //
    // types of standalone attributes are from
    // /net/sched/sch_netem.c

    // tc_netem_qopt
    let kind = TCA_NETEM_UNSPEC;
    let mut value = vec![0; 20];
    //__u32	latency;	/* added delay (us) */
    // 1st in tc_netem_qopts struct but not parsed?
    //__u32   limit;		/* fifo limit (packets) */
    NativeEndian::write_u32(&mut value[0..4], limit as u32);
    //__u32	loss;		/* random packet loss (0=none ~0=100%) */
    NativeEndian::write_u32(&mut value[4..8],
        (u32::MAX as f32 * (loss as f32 / 100.0)) as u32);
    //__u32	gap;		/* re-ordering gap (0 for none) */
    NativeEndian::write_u32(&mut value[8..12], 0 as u32);
    //__u32   duplicate;	/* random packet dup  (0=none ~0=100%) */
    NativeEndian::write_u32(&mut value[12..16], 0 as u32);
    //__u32	jitter;		/* random jitter in latency (us) */
    // skewed by like << 6 bitshift or sth idk
    NativeEndian::write_u32(&mut value[16..20], 0 as u32);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // tc_netem_corr
    let kind = TCA_NETEM_CORR;
    let mut value = vec![0; 12];
    //__u32	delay_corr;	/* delay correlation */
    NativeEndian::write_u32(&mut value[0..4], 0 as u32);
    //__u32	loss_corr;	/* packet loss correlation */
    NativeEndian::write_u32(&mut value[4..8], 0 as u32);
    //__u32	dup_corr;	/* duplicate correlation  */
    NativeEndian::write_u32(&mut value[8..12], 0 as u32);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // distribution data for delays
    // as a vector of i16
    let kind = TCA_NETEM_DELAY_DIST;
    let mut value = vec![0; 2 * distribution.len()];
    let mut start = 0;
    for entry in distribution {
        NativeEndian::write_i16(&mut value[start..start+2], entry.clone());
        start += 2;
    }
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // tc_netem_reorder
    let kind = TCA_NETEM_REORDER;
    let mut value = vec![0; 8];
    //__u32	probability;
    NativeEndian::write_u32(&mut value[0..4], 0 as u32);
    //__u32	correlation;
    NativeEndian::write_u32(&mut value[4..8], 0 as u32);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // tc_netem_corrupt {
    let kind = TCA_NETEM_CORRUPT;
    let mut value = vec![0; 8];
    //__u32	probability;
    NativeEndian::write_u32(&mut value[0..4], 0 as u32);
    //__u32	correlation;
    NativeEndian::write_u32(&mut value[4..8], 0 as u32);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // tc_netem_rate
    let kind = TCA_NETEM_RATE;
    let mut value = vec![0; 16];
    //__u32	rate;	/* byte/s */
    NativeEndian::write_u32(&mut value[0..4], 0 as u32);
    //__s32	packet_overhead;
    NativeEndian::write_i32(&mut value[4..8], 0 as i32);
    //__u32	cell_size;
    NativeEndian::write_u32(&mut value[8..12], 0 as u32);
    //__s32	cell_overhead;
    NativeEndian::write_i32(&mut value[12..16], 0 as i32);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // tc_netem_slot
    let kind = TCA_NETEM_SLOT;
    let mut value = vec![0; 42];
    //__s64   min_delay; /* nsec */
    NativeEndian::write_i64(&mut value[0..8], 0 as i64);
    //__s64   max_delay;
    NativeEndian::write_i64(&mut value[8..16], 0 as i64);
    //__s32   max_packets;
    NativeEndian::write_i32(&mut value[16..22], 0 as i32);
    //__s32   max_bytes;
    NativeEndian::write_i32(&mut value[22..26], 0 as i32);
    //__s64	dist_delay; /* nsec */
    NativeEndian::write_i64(&mut value[26..34], 0 as i64);
    //__s64	dist_jitter; /* nsec */
    NativeEndian::write_i64(&mut value[34..42], 0 as i64);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // 64 bit version of rate in byte/s
    // max of tc_netem_rate.rate and this is picked
    // (meaning always this because the prior is 0)
    let kind = TCA_NETEM_RATE64;
    let mut value = vec![0; 8];
    NativeEndian::write_u64(&mut value[0..8], rate as u64);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // 64 bit version of latency in ns
    // considering tc_netem_qopt.latency doesn't seem to exist anymore
    // (makes nla invalid) just use this
    // why is it signed? I have no idea
    let kind = TCA_NETEM_LATENCY64;
    let mut value = vec![0; 8];
    NativeEndian::write_i64(&mut value[0..8], latency as i64);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // 64 bit version of jitter in ns
    // same as TCA_NETEM_LATENCY64 this is the more supported version
    let kind = TCA_NETEM_JITTER64;
    let mut value = vec![0; 8];
    NativeEndian::write_i64(&mut value[0..8], jitter as i64);
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    // add options and make request
    request.message_mut().attributes.push(TcAttribute::Options(options));
    let _ = request.execute().await.map_err(|e| e.to_string())?;

    // print status
    println!("[qdisc][netem][{}][{}] \
        limit: {} pkts, loss: {}%, rate: {} byte/s, \
        latency: {} ns, jitter: {} ns",
        interface_id,
        if inplace { "changed" } else { "replaced" },
        limit, loss, rate, latency, jitter
    );

    Ok(())
}

/**
 * Replace/Create a default fq_codel on interface
 * @param handle        Handle for rtnetlink
 * @param interface_id  ID of the interface
 */
pub async fn qdisc_fq_codel(
    handle: Handle,
    interface_id: u32
) -> Result<(), String> {
    let mut request = handle
        .qdisc()
        .replace(interface_id as i32)
        .root();

    request.message_mut().attributes.push(
        TcAttribute::Kind(String::from("fq_codel")));

    // make request
    let _ = request.execute().await.map_err(|e| e.to_string())?;

    // print status
    println!("[qdisc][fq_codel][replaced] default");
    Ok(())
}

/**
 * Upper bound on size of distribution
 * really (TCA_BUF_MAX - other headers) / sizeof (__s16)
 */
const MAX_DIST: usize = 16*1024;

/**
 * Parse a iproute2 distribution file into a vector of numbers
 * essentially a port of https://github.com/iproute2/iproute2/blob/v6.13.0/tc/q_netem.c#L125
 * @param path  Path to distribution file e.g. /lib64/tc/pareto.dist
 */
pub async fn get_distribution(
    path: String
) -> Result<Vec<i16>, String> {
    let file = File::open(path.clone()).map_err(|e| e.to_string())?;
    let mut lines = io::BufReader::new(file).lines();

    let mut data: Vec<i16> = Vec::new();
    while let Some(line) = lines.next() {
        let line = String::from(line.unwrap());

        // skip empty and comment lines
        match line.chars().next().unwrap() {
            '\n' | '#' => continue,
            _ => {},
        }
        
        // collect entries
        let mut entries = line.split_whitespace();
        while let Some(entry) = entries.next() {
            if data.len() >= MAX_DIST {
                return Err(format!(
                        "Too much data in {}, got {} entries, max is {}",
                        path.clone(), data.len(), MAX_DIST
                ));
            }
            let parsed = i16::from_str(entry);
            if parsed.is_err() {
                return Err(format!("Could not parse in from {}", entry));
            }
            data.push(parsed.unwrap());
        }
    }
    Ok(data)
}
