use std::{
    net::{IpAddr, SocketAddr},
    num::NonZeroU16,
};

mod addresses;
mod parser;
mod ports;
mod tunnels;

pub use addresses::*;
use inlined::CompactVec;
pub use parser::*;
use portal_tunneler_proto::shared::TunnelSpec;
pub use ports::*;
pub use tunnels::*;

/// The default amount of lanes (sequential ports) to use when hole-punching.
pub const DEFAULT_LANE_COUNT: NonZeroU16 = unsafe { NonZeroU16::new_unchecked(5) };

/// The default port to use when using direct (not hole-punched) connection.
pub const DEFAULT_PORT: u16 = 5995;

/// Gets a small string with this program's name and version.
pub fn get_version_string() -> String {
    format!(
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), " ({} {})"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Gets a string with this program's help documentation.
pub fn get_help_string() -> &'static str {
    // TODO: Write help menu
    "I need somebody"
}

/// The result of parsing the program's arguments.
#[derive(Debug, PartialEq)]
pub enum ArgumentsRequest {
    /// Print the help menu to stdout and exit.
    Help,

    /// Print this program's version to stdout and exit.
    Version,

    /// Run with the provided arguments.
    Run(StartupArguments),
}

/// Specifies the information on how the program should run.
#[derive(Debug, PartialEq)]
pub struct StartupArguments {
    /// Whether to print additional information to stdout.
    pub verbose: bool,

    /// Whether to not print any information to stdout.
    pub silent: bool,

    /// The method to use for connecting to the remote peer.
    pub connect_method: ConnectMethod,

    /// Whether to run in client or server mode.
    pub startup_mode: StartupMode,
}

/// Specifies how to connect to the remote peer, or how a remote peer will connect to us.
#[derive(Debug, PartialEq)]
pub enum ConnectMethod {
    /// Connect to the remote peer via direct communication. This means attempting to connect to
    /// these socket addresses (client), or listening on them for incoming connections (server).
    Direct(CompactVec<2, SocketAddr>),

    /// Connect to the remote peer via hole-punching.
    Punch(PunchConfig),
}

/// Specifies configuration for hole-punching.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PunchConfig {
    /// Our publicly-visible IP address. If `None`, then it will be queried with a public API.
    pub my_ip: Option<IpAddr>,

    /// The first port to try to bind. If `None`, a random port will be requested to the OS.
    pub port_start: Option<NonZeroU16>,

    /// The amount of sequential ports to bind.
    pub lane_count: NonZeroU16,
}

impl PunchConfig {
    pub const fn new() -> Self {
        Self {
            my_ip: None,
            port_start: None,
            lane_count: DEFAULT_LANE_COUNT,
        }
    }
}

/// Specifies whether the program should start in client or server mode.
#[derive(Debug, PartialEq)]
pub enum StartupMode {
    Server(StartServerConfig),
    Client(StartClientConfig),
}

impl StartupMode {
    pub fn is_server(&self) -> bool {
        matches!(self, StartupMode::Server(_))
    }

    pub fn is_client(&self) -> bool {
        matches!(self, StartupMode::Client(_))
    }
}

/// Specifies configuration when starting in server mode.
#[derive(Debug, PartialEq)]
pub struct StartServerConfig {}

impl StartServerConfig {
    pub const fn new() -> Self {
        Self {}
    }
}

/// Specifies configuration when starting in client mode.
#[derive(Debug, PartialEq)]
pub struct StartClientConfig {
    /// The tunnels to open.
    pub tunnels: Vec<TunnelSpec>,
}

impl StartClientConfig {
    pub const fn new() -> Self {
        Self { tunnels: Vec::new() }
    }
}

impl StartupArguments {
    pub const fn new(verbose: bool, silent: bool, connect_method: ConnectMethod, startup_mode: StartupMode) -> Self {
        Self {
            verbose,
            silent,
            connect_method,
            startup_mode,
        }
    }
}
