use rtnetlink::{Handle, QDiscNewRequest};
use netlink_packet_utils::{byteorder::{ByteOrder, NativeEndian}, nla::DefaultNla, traits::Emitable};
use netlink_packet_route::{link::LinkMessageBuffer, tc::{TcAttribute, TcHandle, TcMessage, TcOption}};
use netlink_packet_core::NLM_F_REQUEST;
use futures::TryStreamExt;

/**
 * A working handle must be created in a tokio runtime like:
 * let (connection, handle, _) = rtnetlink::new_connection().unwrap();
 * tokio::spawn(connection);
 */
pub async fn get_interface_id_by_name(handle: Handle, name: String) -> Result<u32, String> {
    let mut response = handle
        .link()
        .get()
        .match_name(name.clone())
        .execute();

    let mut link_index = None;
    while let Some(msg) = response.try_next().await.unwrap() {
        // convert LinkMessage to LinkMessageBuffer via intermediate buffer
        // there's probably a better way to do this but I couldn't find it
        let mut buf = vec![0; msg.buffer_len()];
        msg.emit(&mut buf);
        let msgbuf = LinkMessageBuffer::new(buf);
        link_index = Some(msgbuf.link_index());
    }

    if link_index.is_none() {
        return Err(format!("Could not find interface {}", name.clone()));
    }
    Ok(link_index.unwrap())
}


// from /usr/include/linux/pkt_sched.h
// const TCA_NETEM_CORR: u16 = 1;
// const TCA_NETEM_DELAY_DIST: u16 = 2;
// const TCA_NETEM_REORDER: u16 = 3;
// const TCA_NETEM_CORRUPT: u16 = 4;
const TCA_NETEM_LOSS: u16 = 5;
const TCA_NETEM_RATE: u16 = 6;
// const TCA_NETEM_ECN: u16 = 7;
// const TCA_NETEM_RATE64: u16 = 8;
// const TCA_NETEM_PAD: u16 = 9;
// const TCA_NETEM_LATENCY64: u16 = 10;
// const TCA_NETEM_JITTER64: u16 = 11;
// const TCA_NETEM_SLOT: u16 = 12;
// const TCA_NETEM_SLOT_DIST: u16 = 13;
// const TCA_NETEM_PRNG_SEED: u16 = 14;

pub async fn replace_interface_qdisc_netem(
    handle: Handle,
    interface_id: u32,
    loss: u32,
    
) -> Result<(), String> {
    let mut request = handle
        .qdisc()
        .replace(interface_id as i32)
        .root();

    request.message_mut().attributes.push(
        TcAttribute::Kind(String::from("netem")));


    // add options
    let mut options: Vec<TcOption> = Vec::new();

    // tc_netem_qopts 
    let kind = 0;
    let mut value = vec![0; 20];
    //__u32	latency;	/* added delay (us) */
    // 1st in tc_netem_qopts struct but not parsed?
    //__u32   limit;		/* fifo limit (packets) */
    NativeEndian::write_u32(&mut value[0..4], 0 as u32);
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

    // tc_netem_rate
    let kind = TCA_NETEM_RATE;
    let mut value = vec![0; 16];
    //__u32	rate;	/* byte/s */
    NativeEndian::write_u32(&mut value[0..4], 3000 as u32);
    //__s32	packet_overhead;
    //__u32	cell_size;
    //__s32	cell_overhead;
    options.push(TcOption::Other(DefaultNla::new(kind, value)));

    request.message_mut().attributes.push(TcAttribute::Options(options));

    let _ = request.execute().await.map_err(|e| e.to_string())?;

    println!("Successfully set qdisc");
    Ok(())
}
