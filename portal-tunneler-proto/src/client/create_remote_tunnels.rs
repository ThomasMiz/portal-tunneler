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
    collections::{HashMap, VecDeque},
    io,
    rc::Rc,
};

use quinn::{RecvStream, SendStream};
use tokio::try_join;

use crate::{
    args::{TunnelSpec, TunnelTarget},
    tunnel_proto::{
        remote_tunnels::{StartRemoteTunnelRequestRef, StartRemoteTunnelResponse, TunnelTargetType},
        requests::ClientStreamRequest,
        serialize::{ByteRead, ByteWrite},
    },
};

use super::state::ClientState;

struct CreateRemoteTunnelsState {
    remote_tunnel_specs: VecDeque<TunnelSpec>,
    remote_tunnel_ids: VecDeque<u32>,
}

impl CreateRemoteTunnelsState {
    pub fn new(remote_tunnel_specs: Vec<TunnelSpec>) -> Self {
        Self {
            remote_tunnel_specs: VecDeque::from(remote_tunnel_specs),
            remote_tunnel_ids: VecDeque::new(),            
        }
    }

    pub fn pedro(&mut self) {
        
    }
}

async fn send_tunnel_specs_task(send_stream: &mut SendStream, remote_tunnel_specs: &[TunnelSpec]) -> io::Result<()> {
    for (i, spec) in remote_tunnel_specs.iter().enumerate() {
        let target_type = match spec.target {
            TunnelTarget::Address(_) => TunnelTargetType::Static,
            TunnelTarget::Socks => TunnelTargetType::Socks,
        };

        let request = StartRemoteTunnelRequestRef::new(i as u32, target_type, spec.listen_address.as_ref());
        request.write(send_stream).await?;
    }

    send_stream.finish().await?;
    Ok(())
}

async fn receive_tunnel_results(recv_stream: &mut RecvStream, remote_tunnel_specs: &[TunnelSpec]) -> io::Result<HashMap<u32, TunnelSpec>> {
    let mut map = HashMap::new();

    for (i, spec) in remote_tunnel_specs.iter().enumerate() {
        let response = StartRemoteTunnelResponse::read(recv_stream).await?;
        match response.result {
            Ok(()) => {
                map.insert(i as u32, spec.clone()); // <-- TODO: This clone could be avoided
            }
            Err(error) => eprintln!("Couldn't start remote tunnel {}, server responded with error: {error}", spec.index),
        }
    }

    Ok(map)
}

pub async fn start_remote_tunnels(state: Rc<ClientState>, remote_tunnel_specs: Vec<TunnelSpec>) -> io::Result<()> {
    if remote_tunnel_specs.is_empty() {
        return Ok(());
    }

    let remote_tunnel_specs = VecDeque::from(remote_tunnel_specs);

    let (mut send_stream, mut recv_stream) = state.connection.open_bi().await?;
    ClientStreamRequest::StartRemoteTunnels.write(&mut send_stream).await?;

    let (_, map) = try_join!(
        send_tunnel_specs_task(&mut send_stream, &remote_tunnel_specs),
        receive_tunnel_results(&mut recv_stream, &remote_tunnel_specs),
    )?;

    Ok(())
}
