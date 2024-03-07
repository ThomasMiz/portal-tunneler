mod packet;
mod state;
mod state_machine;

use std::io;
use std::io::Error;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::num::NonZeroU16;
use std::time::Duration;
use std::time::Instant;

use state_machine::StateMachineNode;
use state_machine::TransitionRequest;

pub use crate::packet::*;
pub use crate::state::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendInfo {
    pub from_port: NonZeroU16,
    pub to: SocketAddr,
    pub length: usize,
}

#[derive(Debug)]
pub struct Lane {
    state: LaneState,
    needs_send: bool,
}

impl Lane {
    pub const fn new() -> Self {
        Self {
            state: LaneState::new(),
            needs_send: true,
        }
    }
}

impl Default for Lane {
    fn default() -> Self {
        Self::new()
    }
}

/// The possible actions the user of a puncher state machine should do after polling the machine.
#[derive(Debug)]
pub enum PuncherAction {
    /// Wait for new packets to arrive or for the next timer tick.
    Wait,

    /// (client only) Connect to the destination from the `local` port to the `remote` port. All
    /// other sockets, apart from the one with port `local`, can be closed, and the state machine
    /// can be dropped.
    Connect(Ports),

    /// (server only) Start listening at the `local` port for incoming connections from the
    /// `remote` port. All other sockets, apart from the one with port `local`, can be closed.
    /// However, the state machine must be kept going. It doesn't need to receive packets past this
    /// point, but it does need to keep sending out packets until the client has started talking in
    /// the next protocol. Once that happens, the state machine can be dropped.
    Listen(Ports),

    /// All the lanes have been blocked.
    Failed,

    /// A link failed to be established and a lane to be selected before the timeout expired.
    ///
    /// Note: Once a link is established and a lane has been selected, the timeout is ignored. So
    /// even the state machine is kept going for long after a lane has been selected, it will _not_
    /// time out.
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ports {
    pub local: NonZeroU16,
    pub remote: NonZeroU16,
}

pub struct Puncher {
    my_port_start: NonZeroU16,
    remote_address: IpAddr,
    remote_port_start: NonZeroU16,
    lane_count: NonZeroU16,
    open_lanes_count: u16,
    lanes: Vec<Lane>,
    is_server: bool,
    selected_lane_index: Option<u16>,
    tick_period: Duration,
    next_tick_instant: Instant,
    timeout_instant: Instant,
}

impl Puncher {
    pub fn new(
        is_server: bool,
        my_port_start: NonZeroU16,
        remote_address: IpAddr,
        remote_port_start: NonZeroU16,
        lane_count: NonZeroU16,
        tick_period: Duration,
        timeout: Duration,
    ) -> Self {
        if my_port_start.checked_add(lane_count.get()).is_none() {
            panic!("lane_count would overflow my_port_start");
        }

        if remote_port_start.checked_add(lane_count.get()).is_none() {
            panic!("lane_count would overflow remote_port_start");
        }

        let mut lanes = Vec::with_capacity(lane_count.get() as usize);
        lanes.resize_with(lane_count.get() as usize, Lane::new);

        Self {
            my_port_start,
            remote_address,
            remote_port_start,
            lane_count,
            open_lanes_count: lane_count.get(),
            lanes,
            is_server,
            selected_lane_index: None,
            tick_period,
            next_tick_instant: Instant::now().checked_add(tick_period).unwrap(),
            timeout_instant: Instant::now().checked_add(timeout).unwrap(),
        }
    }

    pub fn my_port_start(&self) -> NonZeroU16 {
        self.my_port_start
    }

    pub fn remote_address(&self) -> IpAddr {
        self.remote_address
    }

    pub fn remote_port_start(&self) -> NonZeroU16 {
        self.remote_port_start
    }

    pub fn lane_count(&self) -> NonZeroU16 {
        self.lane_count
    }

    pub fn open_lanes_count(&self) -> u16 {
        self.open_lanes_count
    }

    pub fn is_server(&self) -> bool {
        self.is_server
    }

    pub fn is_client(&self) -> bool {
        !self.is_server
    }

    pub fn next_tick_instant(&mut self) -> Option<Instant> {
        if (self.is_client() && self.selected_lane_index.is_some()) || self.open_lanes_count == 0 {
            return None;
        }

        let mut next_tick = self.next_tick_instant;

        if self.selected_lane_index.is_none() {
            next_tick = next_tick.min(self.timeout_instant);
        }

        Some(next_tick)
    }

    pub fn tick(&mut self) {
        self.next_tick_instant = self.next_tick_instant.checked_add(self.tick_period).unwrap();

        if let Some(selected_index) = self.selected_lane_index {
            self.lanes[selected_index as usize].needs_send = self.is_server;
        } else {
            for lane in &mut self.lanes {
                lane.needs_send = match lane.state {
                    LaneState::Connecting(_) | LaneState::Establishing(_) => true,
                    LaneState::Selected(_) => self.is_server,
                    LaneState::Blocked(_) | LaneState::Closed => false,
                }
            }
        }
    }

    pub fn received_from<'a>(&mut self, recv_result: io::Result<(&'a [u8], SocketAddr)>, local_port: u16) -> Option<&'a [u8]> {
        // Find the index of the lane from the port number the packet arrived at. If this is not
        // a valid value, panic (since this is wrong usage of the state machine).
        let lane_index = match local_port.checked_sub(self.my_port_start.get()) {
            Some(i) if i < self.lanes.len() as u16 => i,
            _ => panic!(
                "received_from called with an invalid port: {local_port} but port range is {} to {}",
                self.my_port_start,
                self.my_port_start.get() + self.lane_count.get() - 1
            ),
        };

        // Get the lane's state. If the lane is blocked, then we ignore any of its incoming packet.
        let lane = &mut self.lanes[lane_index as usize];
        if !lane.state.is_active() {
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
            || from.port() < self.remote_port_start.get()
            || from.port() >= (self.remote_port_start.get() + self.lane_count.get())
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

        // TODO: Remove!!!
        println!("Packet data has status: {:?}", packet_data.lane_status);

        if packet_data.lane_status == LaneStatus::Blocked {
            self.block_lane(lane_index, BlockReason::BlockedByRemote);
            return Some(packet_data.application_data);
        }

        let has_selected = self.selected_lane_index.is_some();
        let result = lane.state.process_packet(self.is_server, has_selected, packet_data.lane_status);

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
                lane.state = LaneState::Establishing(EstablishingState::new());
                lane.needs_send = true;
            }
            TransitionRequest::Selected => {
                lane.state = LaneState::Selected(SelectedState::new());
                lane.needs_send = self.is_server;
                self.set_selected_lane(lane_index);
            }
        }

        Some(packet_data.application_data)
    }

    pub fn send_to(&mut self, buf: &mut [u8], application_data: &[u8]) -> Option<SendInfo> {
        self.get_next_lane_index_needing_resend().map(|lane_index| {
            let lane = &mut self.lanes[lane_index];
            lane.state.process_sent();

            let length = PacketData::new(lane.state.status(), application_data).write_to(buf);

            SendInfo {
                from_port: self.my_port_start.saturating_add(lane_index as u16),
                to: SocketAddr::new(self.remote_address, self.remote_port_start.get() + lane_index as u16),
                length,
            }
        })
    }

    pub fn send_failed(&mut self, local_port: u16, error: Error) {
        let lane_index = match local_port.checked_sub(self.my_port_start.get()) {
            Some(i) if i < self.lanes.len() as u16 => i,
            _ => panic!(
                "on_send_failed called with an invalid port: {local_port} but port range is {} to {}",
                self.my_port_start,
                self.my_port_start.get() + self.lane_count.get() - 1
            ),
        };

        if self.lanes[lane_index as usize].state.is_active() {
            self.block_lane(lane_index, BlockReason::SendError(error));
        }
    }

    pub fn poll(&self) -> PuncherAction {
        if let Some(selected_index) = self.selected_lane_index {
            let ports = Ports {
                local: self.my_port_start.saturating_add(selected_index),
                remote: self.remote_port_start.saturating_add(selected_index),
            };

            return match self.is_server {
                true => PuncherAction::Listen(ports),
                false => PuncherAction::Connect(ports),
            };
        }

        if self.open_lanes_count == 0 {
            return PuncherAction::Failed;
        }

        if Instant::now() >= self.timeout_instant {
            return PuncherAction::Timeout;
        }

        PuncherAction::Wait
    }

    fn block_lane(&mut self, lane_index: u16, reason: BlockReason) {
        let lane = &mut self.lanes[lane_index as usize];
        if !lane.state.is_active() {
            panic!("Attempted to block inactive lane {lane_index} with reason {reason:?}")
        }

        lane.state = LaneState::Blocked(reason);
        self.open_lanes_count -= 1;
        lane.needs_send = false;
    }

    fn set_selected_lane(&mut self, lane_index: u16) {
        if let Some(selected_index) = self.selected_lane_index {
            panic!("set_selected_lane({lane_index}) called when lane {selected_index} has already been selected");
        }

        self.selected_lane_index = Some(lane_index);

        let lane_index = lane_index as usize;
        for (index, lane) in self.lanes.iter_mut().enumerate() {
            if index == lane_index {
                continue;
            }

            lane.needs_send = false;
            if lane.state.is_active() {
                lane.state = LaneState::Closed;
                self.open_lanes_count -= 1;
            }
        }
    }

    fn get_next_lane_index_needing_resend(&mut self) -> Option<usize> {
        for (lane_index, lane) in self.lanes.iter_mut().enumerate() {
            if lane.needs_send {
                lane.needs_send = false;
                return Some(lane_index);
            }
        }

        None
    }
}
