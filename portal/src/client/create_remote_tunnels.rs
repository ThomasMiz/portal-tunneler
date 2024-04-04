//! This module handles requesting the server starts the remote tunnels. This means opening a
//! bidirectional stream, indicating we will use the stream to start new remote tunnels, sending
//! the requests for each remote tunnel specification and receiving the responses.
//!
//! While a simple implementation that sends all the requests and then waits for all the responses
//! would be fine in most cases, a proper implementation actually requires having both of these
//! tasks being done asynchronously at the same time. This is because if we have too many requests
//! to send, the server will correctly handle and answer them in order, but since we're not reading
//! the server's responses, our receive buffer will eventually fill up, at which point the server
//! will likely stop handling requests until the client starts reading the buffer. This leads to
//! an "over-the-network deadlock", where the client will not read the responses until the server
//! handles the requests and the server will not handle any more requests until the client starts
//! reading the responses.
//!
//! Now, this scenario is very unlikely, given that there have to be a lot of requests or very long
//! responses, or there have to be small enough buffer sizes. Either way, we prefer to handle this
//! properly for both better quality and performance.

use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{self, Error, ErrorKind},
    rc::Rc,
};

use portal_tunneler_proto::{
    client::ClientState,
    serialize::{ByteRead, ByteWrite},
    shared::{ClientStreamRequest, RemoteTunnelID, StartRemoteTunnelRequestRef, StartRemoteTunnelResponse, TunnelSpec},
};
use quinn::{RecvStream, SendStream};
use tokio::try_join;

struct CreateRemoteTunnelsState {
    client: Rc<ClientState>,
    inner: RefCell<CreateRemoteTunnelsStateInner>,
}

struct CreateRemoteTunnelsStateInner {
    remote_tunnel_specs: VecDeque<TunnelSpec>,
    remote_tunnel_ids: VecDeque<RemoteTunnelID>,
}

impl CreateRemoteTunnelsState {
    pub fn new(client: Rc<ClientState>, remote_tunnel_specs: Vec<TunnelSpec>) -> Self {
        let inner = CreateRemoteTunnelsStateInner {
            remote_tunnel_specs: VecDeque::from(remote_tunnel_specs),
            remote_tunnel_ids: VecDeque::new(),
        };

        Self {
            client,
            inner: RefCell::new(inner),
        }
    }
}

async fn send_tunnel_specs_task(operation_state: &CreateRemoteTunnelsState, send_stream: &mut SendStream) -> io::Result<()> {
    loop {
        let (tunnel_id, spec) = {
            let mut state = operation_state.inner.borrow_mut();
            let spec = match state.remote_tunnel_specs.pop_front() {
                Some(spec) => Rc::new(spec),
                None => break,
            };

            let tunnel_id = operation_state.client.lock().register_remote_tunnel(Rc::clone(&spec));
            state.remote_tunnel_ids.push_back(tunnel_id);

            (tunnel_id, spec)
        };

        let request = StartRemoteTunnelRequestRef::new(tunnel_id, spec.target.as_type(), spec.listen_address.as_ref());
        request.write(send_stream).await?;
    }

    send_stream.finish().await?;
    Ok(())
}

async fn receive_tunnel_results(operation_state: &CreateRemoteTunnelsState, recv_stream: &mut RecvStream) -> io::Result<()> {
    loop {
        let response = match StartRemoteTunnelResponse::read(recv_stream).await {
            Ok(resp) => resp,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };

        let tunnel_id = match operation_state.inner.borrow_mut().remote_tunnel_ids.pop_front() {
            None => return Err(Error::new(ErrorKind::Other, "The server sent a response when one wasn't expected")),
            Some(id) => id,
        };

        if let Err(error) = response.result {
            let maybe_spec = operation_state.client.lock().unregister_remote_tunnel(tunnel_id);
            match maybe_spec {
                Some(spec) => eprintln!("Couldn't start remote tunnel {}, server responded with error: {error}", spec.index),
                None => eprintln!("Couldn't start unidentified remote tunnel, server responded with error: {error}"),
            }
        }
    }
}

pub async fn start_remote_tunnels(client: Rc<ClientState>, remote_tunnel_specs: Vec<TunnelSpec>) -> io::Result<()> {
    if remote_tunnel_specs.is_empty() {
        return Ok(());
    }

    let (mut send_stream, mut recv_stream) = client.connection().open_bi().await?;
    ClientStreamRequest::StartRemoteTunnels.write(&mut send_stream).await?;

    let operation_state = CreateRemoteTunnelsState::new(client, remote_tunnel_specs);
    try_join!(
        send_tunnel_specs_task(&operation_state, &mut send_stream),
        receive_tunnel_results(&operation_state, &mut recv_stream),
    )?;

    Ok(())
}
