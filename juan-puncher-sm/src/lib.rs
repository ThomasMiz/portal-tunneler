mod packet;
mod state;
mod state_machine;

use std::cmp::Ordering;
use std::io;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::num::NonZeroU16;
use std::time::Duration;
use std::time::Instant;

use state_machine::StateMachineNode;
use state_machine::TransitionRequest;

pub use crate::packet::*;
pub use crate::state::*;

pub struct SendInfo {
    pub from_port: u16,
    pub to: SocketAddr,
    pub length: usize,
}

#[derive(Debug)]
pub struct Lane {
    state: LaneState,
    needs_send: bool,
}

impl Lane {
    pub fn new() -> Self {
        Self {
            state: LaneState::new(),
            needs_send: true,
        }
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
    pub local: u16,
    pub remote: u16,
}

pub struct Puncher {
    my_address: IpAddr,
    my_port_start: u16,
    remote_address: IpAddr,
    remote_port_start: u16,
    lane_count: NonZeroU16,
    open_lanes_count: u16,
    lanes: Vec<Lane>,
    is_server: bool,
    selected_lane_index: Option<u16>,
    tick_period: Duration,
    last_tick_instant: Instant,
    timeout_instant: Instant,
}

impl Puncher {
    pub fn new(
        is_server: bool,
        my_address: IpAddr,
        my_port_start: u16,
        remote_address: IpAddr,
        remote_port_start: u16,
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

        let ip_comparison = match (my_address, remote_address) {
            (IpAddr::V4(me), IpAddr::V4(other)) => me.cmp(&other),
            (IpAddr::V6(me), IpAddr::V6(other)) => me.cmp(&other),
            _ => panic!("my_address and remote_address are not the same IpAddr variant"),
        };

        if ip_comparison == Ordering::Equal {
            panic!("my_address and remote_address must not be the same");
        }

        let mut lanes = Vec::with_capacity(lane_count.get() as usize);
        lanes.fill_with(|| Lane::new());

        Self {
            my_address,
            my_port_start,
            remote_address,
            remote_port_start,
            lane_count,
            open_lanes_count: lane_count.get(),
            lanes,
            is_server,
            selected_lane_index: None,
            tick_period,
            last_tick_instant: Instant::now(),
            timeout_instant: Instant::now().checked_add(timeout).unwrap(),
        }
    }

    pub fn my_address(&self) -> IpAddr {
        self.my_address
    }

    pub fn my_port_start(&self) -> u16 {
        self.my_port_start
    }

    pub fn remote_address(&self) -> IpAddr {
        self.remote_address
    }

    pub fn remote_port_start(&self) -> u16 {
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

        let next_tick = self.last_tick_instant.checked_add(self.tick_period)?;
        if next_tick >= self.last_tick_instant {
            self.last_tick_instant = next_tick;
        }

        Some(next_tick.min(self.timeout_instant))
    }

    pub fn tick(&mut self) {
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
        if lane.state.is_blocked() {
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
                from_port: self.my_port_start + lane_index as u16,
                to: SocketAddr::new(self.remote_address, self.remote_port_start + lane_index as u16),
                length,
            }
        })
    }

    pub fn poll(&self) -> PuncherAction {
        if let Some(selected_index) = self.selected_lane_index {
            let ports = Ports {
                local: self.my_port_start + selected_index,
                remote: self.remote_port_start + selected_index,
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
        if lane.state.is_blocked() {
            panic!("Attempted to double-lock lane {lane_index} with reason {reason:?}")
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
