use crate::{
    state::{BlockReason, ConnectingState, LaneStatus},
    EstablishingState, LaneState, SelectedState,
};

/// Represents a transition (or remain) request on the puncher state machine.
pub(crate) enum TransitionRequest {
    /// Do not transition. Remain in the current state.
    Remain,

    /// Transition to the `Establishing` state.
    Establishing,

    /// Transition to the `Selected` state.
    Selected,
}

/// A trait for types that represent a node in the puncher state machine.
pub(crate) trait StateMachineNode {
    /// Processes a received packet's lane status. Returns whether to transition to another state,
    /// remain in the same state, or block the lane.
    fn process_packet(
        &mut self,
        is_selector: bool,
        has_selected: bool,
        received_lane_status: LaneStatus,
    ) -> Result<TransitionRequest, BlockReason>;

    /// Processes an outgoing packet having sent from this lane while on this state.
    fn process_sent(&mut self);
}

impl StateMachineNode for ConnectingState {
    fn process_packet(
        &mut self,
        is_selector: bool,
        has_selected: bool,
        received_lane_status: LaneStatus,
    ) -> Result<TransitionRequest, BlockReason> {
        let can_select = !is_selector && !has_selected;

        match received_lane_status {
            LaneStatus::Establishing if !self.sent => Err(BlockReason::UnexpectedTransition),
            LaneStatus::Establishing if can_select => Ok(TransitionRequest::Selected),
            LaneStatus::Connecting | LaneStatus::Establishing => Ok(TransitionRequest::Establishing),
            LaneStatus::Selected => Err(BlockReason::UnexpectedTransition),
            LaneStatus::Blocked => Err(BlockReason::BlockedByRemote),
        }
    }

    fn process_sent(&mut self) {
        self.sent = true;
    }
}

impl StateMachineNode for EstablishingState {
    fn process_packet(
        &mut self,
        is_selector: bool,
        has_selected: bool,
        received_lane_status: LaneStatus,
    ) -> Result<TransitionRequest, BlockReason> {
        let can_select = !is_selector && !has_selected;

        match received_lane_status {
            LaneStatus::Selected if !self.sent || can_select => Err(BlockReason::UnexpectedTransition),
            LaneStatus::Establishing if can_select => Ok(TransitionRequest::Selected),
            LaneStatus::Connecting | LaneStatus::Establishing => Ok(TransitionRequest::Remain),
            LaneStatus::Selected => Ok(TransitionRequest::Selected),
            LaneStatus::Blocked => Err(BlockReason::BlockedByRemote),
        }
    }

    fn process_sent(&mut self) {
        self.sent = true;
    }
}

impl StateMachineNode for SelectedState {
    fn process_packet(
        &mut self,
        is_selector: bool,
        _has_selected: bool,
        received_lane_status: LaneStatus,
    ) -> Result<TransitionRequest, BlockReason> {
        match received_lane_status {
            LaneStatus::Connecting | LaneStatus::Establishing => Ok(TransitionRequest::Remain),
            LaneStatus::Selected if is_selector && !self.sent => Err(BlockReason::UnexpectedTransition),
            LaneStatus::Selected => Ok(TransitionRequest::Remain),
            LaneStatus::Blocked => Err(BlockReason::BlockedByRemote),
        }
    }

    fn process_sent(&mut self) {
        self.sent = true;
    }
}

impl StateMachineNode for LaneState {
    fn process_packet(
        &mut self,
        is_selector: bool,
        has_selected: bool,
        received_lane_status: LaneStatus,
    ) -> Result<TransitionRequest, BlockReason> {
        match self {
            Self::Connecting(t) => t.process_packet(is_selector, has_selected, received_lane_status),
            Self::Establishing(t) => t.process_packet(is_selector, has_selected, received_lane_status),
            Self::Selected(t) => t.process_packet(is_selector, has_selected, received_lane_status),
            Self::Blocked(_) => Ok(TransitionRequest::Remain),
        }
    }

    fn process_sent(&mut self) {
        match self {
            Self::Connecting(t) => t.process_sent(),
            Self::Establishing(t) => t.process_sent(),
            Self::Selected(t) => t.process_sent(),
            Self::Blocked(_) => {}
        }
    }
}
