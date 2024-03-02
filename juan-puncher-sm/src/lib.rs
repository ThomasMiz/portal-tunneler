mod handler;
mod packet;
mod state;
mod state_machine;

use std::cmp::Ordering;
use std::io;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::num::NonZeroU16;

use state_machine::StateMachineNode;
use state_machine::TransitionRequest;

pub use crate::handler::*;
pub use crate::packet::*;
pub use crate::state::*;

pub struct Puncher<H: PuncherHandler> {
    my_address: IpAddr,
    my_port_start: u16,
    remote_address: IpAddr,
    remote_port_start: u16,
    lane_count: NonZeroU16,
    open_lanes_count: u16,
    lanes: Vec<LaneState>,
    is_selector: bool,
    has_selected: bool,
    lanes_needing_resend: Vec<bool>,
    pub handler: H,
}

impl<H: PuncherHandler> Puncher<H> {
    pub fn new(
        my_address: IpAddr,
        my_port_start: u16,
        remote_address: IpAddr,
        remote_port_start: u16,
        lane_count: NonZeroU16,
        handler: H,
    ) -> Self {
        if my_port_start.checked_add(lane_count.get()).is_none() {
            panic!("lane_count would overflow my_port_start");
        }

        if remote_port_start.checked_add(lane_count.get()).is_none() {
            panic!("lane_count would overflow remote_port_start");
        }

        let ip_comparison = match (my_address, remote_address) {
            (IpAddr::V4(me), IpAddr::V4(other)) => me.cmp(&other),
            (IpAddr::V6(me), IpAddr::V6(other)) => me.cmp(&other),
            _ => panic!("my_address and remote_address are not the same IpAddr variant"),
        };

        let is_selector = match ip_comparison {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => panic!("my_address and remote_address must not be the same"),
        };

        let mut lanes = Vec::with_capacity(lane_count.get() as usize);
        lanes.fill_with(|| LaneState::new());

        Self {
            my_address,
            my_port_start,
            remote_address,
            remote_port_start,
            lane_count,
            open_lanes_count: lane_count.get(),
            lanes,
            is_selector,
            has_selected: false,
            lanes_needing_resend: vec![false; lane_count.get() as usize],
            handler,
        }
    }

    pub fn lanes(&self) -> &[LaneState] {
        &self.lanes
    }

    pub fn open_lanes_count(&self) -> u16 {
        self.open_lanes_count
    }

    pub fn timer_tick(&mut self) {
        self.lanes_needing_resend.fill(true);
    }

    pub fn received_from<'a>(&mut self, recv_result: io::Result<(&'a [u8], SocketAddr)>, to_port: u16) -> Option<&'a [u8]> {
        // Find the index of the lane from the port number the packet arrived at. If this is not
        // a valid value, panic (since this is wrong usage of the state machine).
        let lane_index = match to_port.checked_sub(self.my_port_start) {
            Some(i) if i < self.lanes.len() as u16 => i,
            _ => panic!(
                "received_from called with an invalid port: {to_port} but port range is {} to {}",
                self.my_port_start,
                self.my_port_start + self.lane_count.get() - 1
            ),
        };

        // Get the lane's state. If the lane is blocked, then we ignore any of its incoming packet.
        let lane = &mut self.lanes[lane_index as usize];
        if lane.is_blocked() {
            return None;
        }

        let (buf, from) = match recv_result {
            Ok(t) => t,
            Err(error) => {
                self.block_lane(lane_index, BlockReason::ReceiveError(error));
                return None;
            }
        };

        // If the packet's source IP or port is not what we expect, block the lane due to interference.
        if from.ip() != self.remote_address
            || from.port() < self.remote_port_start
            || from.port() >= (self.remote_port_start + self.lane_count.get())
        {
            self.block_lane(lane_index, BlockReason::Interference(from));
            return None;
        }

        // Parse the packet's data. If the format is wrong, block the lane due to a bad packet.
        let packet_data = match PacketData::parse(buf) {
            Ok(p) => p,
            Err(packet_error) => {
                self.block_lane(lane_index, BlockReason::BadPacket(packet_error));
                return None;
            }
        };

        if packet_data.lane_status == LaneStatus::Blocked {
            self.block_lane(lane_index, BlockReason::BlockedByRemote);
            return Some(packet_data.application_data);
        }

        let result = lane.process_packet(self.is_selector, self.has_selected, packet_data.lane_status);
        let transition = match result {
            Ok(t) => t,
            Err(block_reason) => {
                self.block_lane(lane_index, block_reason);
                return Some(packet_data.application_data);
            }
        };

        match transition {
            TransitionRequest::Remain => {}
            TransitionRequest::Establishing => {
                *lane = LaneState::Establishing(EstablishingState::new());
            }
            TransitionRequest::Selected => {
                *lane = LaneState::Selected(SelectedState::new());
            }
        }

        Some(packet_data.application_data)
    }

    fn block_lane(&mut self, lane_index: u16, reason: BlockReason) {
        let lane = &mut self.lanes[lane_index as usize];
        if lane.is_blocked() {
            return;
        }

        *lane = LaneState::Blocked(reason);
        self.open_lanes_count -= 1;
    }
}
