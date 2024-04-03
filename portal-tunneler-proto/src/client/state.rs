use std::{cell::RefCell, collections::HashMap, rc::Rc};

use quinn::Connection;

use crate::shared::{RemoteTunnelID, TunnelSpec};

/// Stores information about the client's current state.
pub struct ClientState {
    pub connection: Connection,
    pub inner: RefCell<ClientStateInner>,
}

impl ClientState {
    /// Constructs a new [`ClientState`] with the given connection and no information.
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            inner: RefCell::new(ClientStateInner::new()),
        }
    }
}

/// The inner part of [`ClientState`] which stores any variable information on the client's state.
pub struct ClientStateInner {
    /// A counter for remote tunnel IDs.
    remote_tunnel_next_id: RemoteTunnelID,

    /// A map with the information of the remote tunnels organized by remote tunnel ID.
    remote_tunnels: HashMap<RemoteTunnelID, Rc<TunnelSpec>>,
}

impl ClientStateInner {
    fn new() -> Self {
        Self {
            remote_tunnel_next_id: RemoteTunnelID(0),
            remote_tunnels: HashMap::new(),
        }
    }

    /// Assigns a remote tunnel ID to the given tunnel specification.
    ///
    /// This makes it immediately available through
    /// [`get_remote_tunnel`](Self::get_remote_tunnel), even if the server hasn't yet confirmed the
    /// tunnel has been started.
    ///
    /// The reason why we don't wait for the tunnel to be confirmed is because if the server opens
    /// a new tunnel connection with said ID, then that can be taken as confirmation that the
    /// tunnel has been opened. This can happen before the tunnel is confirmed because QUIC streams
    /// are asynchronous and independent.
    ///
    /// To de-register a remote tunnel, use
    /// [`Self::unregister_remote_tunnel`](Self::unregister_remote_tunnel).
    pub fn register_remote_tunnel(&mut self, specification: Rc<TunnelSpec>) -> RemoteTunnelID {
        loop {
            let id = self.remote_tunnel_next_id;
            self.remote_tunnel_next_id.0 += 1;

            let entry = self.remote_tunnels.entry(id);

            if let std::collections::hash_map::Entry::Vacant(vacant_entry) = entry {
                vacant_entry.insert(specification);
                return id;
            }
        }
    }

    /// Gets a remote tunnel's specification by ID.
    ///
    /// Returns [`Some`] with the tunnel's specification if found, or [`None`] if there's no tunnel
    /// specification for that ID.
    pub fn get_remote_tunnel(&self, id: RemoteTunnelID) -> Option<Rc<TunnelSpec>> {
        self.remote_tunnels.get(&id).cloned()
    }

    /// Unregisters a remote tunnel ID, freeing it up for possible future use.
    ///
    /// Returns [`Some`] with the tunnel's specification if found, or [`None`] if there was no
    /// tunnel specification for that ID.
    pub fn unregister_remote_tunnel(&mut self, id: RemoteTunnelID) -> Option<Rc<TunnelSpec>> {
        self.remote_tunnels.remove(&id)
    }
}
