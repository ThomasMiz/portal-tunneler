use std::{io::Error, net::SocketAddr};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::packet::PacketDataError;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, IntoPrimitive, TryFromPrimitive)]
pub enum LaneStatus {
    /// The lane is attempting to connect, but hasn’t heard back from the remote peer.
    #[default]
    Connecting = 1,

    /// The lane has heard heard back from the remote peer (and thus its outgoing packets are now
    /// indicating to the remote host that _"I can hear you in here!"_)
    Establishing = 2,

    /// The lane has heard an `Establishing` packet back from the remote peer. This means _"I can
    /// hear them, and they can hear me too"_. The connection is established and the lane has been
    /// selected as the link for the communication. Only one lane can be selected.
    Selected = 3,

    /// The system detected interference, bad packets, or garbage data, and thus decided this lane
    /// is unsuitable for use.
    Blocked = 255,
}

#[derive(Debug)]
pub enum LaneState {
    Connecting(ConnectingState),
    Establishing(EstablishingState),
    Selected(SelectedState),
    Blocked(BlockReason),
}

impl Default for LaneState {
    fn default() -> Self {
        Self::new()
    }
}

impl LaneState {
    pub const fn new() -> Self {
        Self::Connecting(ConnectingState::new())
    }

    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting(_))
    }

    pub fn is_establishing(&self) -> bool {
        matches!(self, Self::Establishing(_))
    }

    pub fn is_selected(&self) -> bool {
        matches!(self, Self::Selected(_))
    }

    pub fn is_open(&self) -> bool {
        !self.is_blocked()
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConnectingState {
    /// Whether any packet was sent since the lane got to this state.
    pub sent: bool,
}

impl ConnectingState {
    pub const fn new() -> Self {
        Self { sent: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EstablishingState {
    /// Whether any packet was sent since the lane got to this state.
    pub sent: bool,
}

impl EstablishingState {
    pub const fn new() -> Self {
        Self { sent: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectedState {
    /// Whether any packet was sent since the lane got to this state.
    pub sent: bool,

    /// Whether a packet was received with the `Selected` status on this lane.
    pub received_selected: bool,
}

impl SelectedState {
    pub const fn new() -> Self {
        Self {
            sent: false,
            received_selected: false,
        }
    }
}

#[derive(Debug)]
pub enum BlockReason {
    /// An IO error occurred while receiving data on this lane.
    ReceiveError(Error),

    /// An IO error occurred while sending data on this lane.
    SendError(Error),

    /// A packet was received whose data had an invalid format.
    BadPacket(PacketDataError),

    /// A packet was received from a wrong source IP or port.
    Interference(SocketAddr),

    /// A packet with the `Blocked` status was received.
    BlockedByRemote,

    /// A packet with an unexpected status for the lane's current state was received. This can
    /// happen, for example, if a lane receives a `Selected` without having sent an `Establishing`
    /// (or without having been `Establishing`).
    UnexpectedTransition,
}
