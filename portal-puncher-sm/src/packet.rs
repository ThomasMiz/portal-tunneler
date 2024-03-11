//! Format of a portal-puncher UDP packet:
//! +----------+-------------+------------------+
//! | PREAMBLE | LANE_STATUS | APPLICATION_DATA |
//! +----------+-------------+------------------+
//! |    8     |      1      |     VARIABLE     |
//! +----------+-------------+------------------+
//!
//! The `PREAMBLE` is an 8-byte sequence that's always expected to be the same. This allows quickly
//! recognizing when a packet clearly is not in the right format.
//!
//! The current `PROTOCOL_VERSION` is 1. The `LANE_STATUS` indicates the sender's status on that
//! lane, its possible values are determined by the [`LaneStatus`] enum. Finally, the
//! `APPLICATION_DATA` is an arbitrary value whose size is the remaining bytes of the payload,
//! that is specified by the application operating on top of the hole puncher.

use crate::state::LaneStatus;

/// The maximum size of a UDP payload one can reasonably expect to be deliverable over the network.
pub const MAX_REASONABLE_PAYLOAD: usize = 1400;

/// The maximum size of a UDP payload one can reasonably expect to be deliverable over the network
/// without fragmentation, leaving some extra space in case any more headers are added in transit.
pub const MAX_RECOMMENDED_PAYLOAD: usize = 1350;

/// The maximum size of a UDP payload that can be delivered over the network guaranteeing no
/// fragmentation occurs.
/// https://stackoverflow.com/questions/1098897/what-is-the-largest-safe-udp-packet-size-on-the-internet
pub const MAX_FRAGMENTATION_SAFE_PAYLOAD: usize = 508;

/// The byte sequence all packets are expected to start with.
pub const PREAMBLE: [u8; 8] = [0x38, 0x08, 0x42, 0x8b, 0x11, 0x39, 0x42, 0x53];

/// The size (in bytes) of the [`PREAMBLE`].
pub const PREAMBLE_SIZE: usize = PREAMBLE.len();

/// The size (in bytes) the lane status.
pub const LANE_STATUS_SIZE: usize = 1;

/// The size (in bytes) of the puncher packet header.
pub const PACKET_HEADER_SIZE: usize = PREAMBLE_SIZE + LANE_STATUS_SIZE;

/// The maximum reasonable user data size on a punch packet.
pub const MAX_REASONABLE_APPLICATION_DATA: usize = MAX_REASONABLE_PAYLOAD - PACKET_HEADER_SIZE;

/// The maximum recommended user data size on a punch packet.
pub const MAX_RECOMMENDED_APPLICATION_DATA: usize = MAX_RECOMMENDED_PAYLOAD - PACKET_HEADER_SIZE;

/// The maximum fragmentation-safe user data size on a punch packet.
pub const MAX_FRAGMENTATION_SAFE_APPLICATION_DATA: usize = MAX_FRAGMENTATION_SAFE_PAYLOAD - PACKET_HEADER_SIZE;

pub(crate) struct PacketData<'a> {
    pub lane_status: LaneStatus,
    pub application_data: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDataError {
    /// The packet's payload is too short to be valid.
    PacketTooShort,

    /// The packet's preamble does not match.
    WrongPreamble,

    /// The packet has an invalid lane status byte.
    InvalidLaneStatus,
}

impl<'a> PacketData<'a> {
    pub fn new(lane_status: LaneStatus, application_data: &'a [u8]) -> Self {
        if application_data.len() > MAX_REASONABLE_APPLICATION_DATA {
            panic!("application_data is over the allowed size limit of {MAX_REASONABLE_APPLICATION_DATA}");
        }

        Self {
            lane_status,
            application_data,
        }
    }

    pub fn write_to(&self, buf: &mut [u8]) -> usize {
        if buf.len() < PACKET_HEADER_SIZE + self.application_data.len() {
            panic!("The provided buffer is not large enough to write this PacketData");
        }

        buf[0..PREAMBLE_SIZE].copy_from_slice(&PREAMBLE);
        let mut index = PREAMBLE_SIZE;

        buf[index] = self.lane_status.into_u8();
        index += LANE_STATUS_SIZE;

        buf[index..(index + self.application_data.len())].copy_from_slice(self.application_data);
        index += self.application_data.len();

        index
    }

    pub fn parse(buf: &'a [u8]) -> Result<Self, PacketDataError> {
        if buf.len() < PACKET_HEADER_SIZE {
            return Err(PacketDataError::PacketTooShort);
        }

        if buf[..PREAMBLE_SIZE] != PREAMBLE {
            return Err(PacketDataError::WrongPreamble);
        }

        let mut index = PREAMBLE_SIZE;

        let lane_status = LaneStatus::from_u8(buf[index]).ok_or(PacketDataError::InvalidLaneStatus)?;
        index += 1;

        Ok(Self {
            lane_status,
            application_data: &buf[index..],
        })
    }
}
