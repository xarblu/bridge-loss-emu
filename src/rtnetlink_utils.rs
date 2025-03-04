use rtnetlink::Handle;
use netlink_packet_utils::traits::Emitable;
use netlink_packet_route::link::LinkMessageBuffer;
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
        println!("{}", msgbuf.link_index());
        link_index = Some(msgbuf.link_index());
    }

    if link_index.is_none() {
        return Err(format!("Could not find interface {}", name.clone()));
    }
    Ok(link_index.unwrap())
}

