use std::{io::Error, net::SocketAddr};

use crate::packet::PacketDataError;

/// Represents the possible statuses for a lane.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub(crate) enum LaneStatus {
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

impl LaneStatus {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Connecting),
            2 => Some(Self::Establishing),
            3 => Some(Self::Selected),
            255 => Some(Self::Blocked),
            _ => None,
        }
    }

    pub fn into_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug)]
pub enum LaneState {
    Connecting(ConnectingState),
    Establishing(EstablishingState),
    Selected(SelectedState),
    Closed,
    Blocked(BlockReason),
}

impl Default for LaneState {
    fn default() -> Self {
        Self::new()
    }
}

impl LaneState {
    /// Creates a new [`LaneState`] in the initial `Connecting` state.
    pub const fn new() -> Self {
        Self::Connecting(ConnectingState::new())
    }

    /// Gets whether this lane is connecting.
    pub fn is_connecting(&self) -> bool {
        matches!(self, Self::Connecting(_))
    }

    /// Gets whether this lane is establishing.
    pub fn is_establishing(&self) -> bool {
        matches!(self, Self::Establishing(_))
    }

    /// Gets whether this lane is selected.
    pub fn is_selected(&self) -> bool {
        matches!(self, Self::Selected(_))
    }

    /// Gets whether this lane is closed.
    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Gets whether this lane is blocked.
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }

    /// Gets whether this lane is still active. That is, if it's not blocked nor closed.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connecting(_) | Self::Establishing(_) | Self::Selected(_))
    }

    /// Gets this lane's status.
    ///
    /// # Panics
    /// Panics if this lane is [`LaneState::Closed`], as that state has no corresponding status.
    pub(crate) fn status(&self) -> LaneStatus {
        match self {
            Self::Connecting(_) => LaneStatus::Connecting,
            Self::Establishing(_) => LaneStatus::Establishing,
            Self::Selected(_) => LaneStatus::Selected,
            Self::Closed => panic!("Cannot get status of closed lane"),
            Self::Blocked(_) => LaneStatus::Blocked,
        }
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
}

impl SelectedState {
    pub const fn new() -> Self {
        Self { sent: false }
    }
}

/// The possible reason for which a lane may get blocked.
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
