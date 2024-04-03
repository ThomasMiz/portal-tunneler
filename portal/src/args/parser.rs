use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use portal_tunneler_proto::shared::TunnelSide;

use super::{
    parse_ip_addr_arg, parse_lane_count_arg, parse_port_number_arg, parse_socket_arg, parse_tunnel_spec_arg, ArgumentsRequest,
    ConnectMethod, IpAddrErrorType, LaneCountErrorType, PortErrorType, PunchConfig, SocketErrorType, StartClientConfig, StartServerConfig,
    StartupArguments, StartupMode, TunnelSpecErrorType, DEFAULT_PORT,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsError {
    UnknownArgument(String),
    ConnectError(SocketErrorType),
    ListenError(SocketErrorType),
    MyIpError(IpAddrErrorType),
    LaneCount(LaneCountErrorType),
    PortStart(PortErrorType),
    LocalTunnel(TunnelSpecErrorType),
    RemoteTunnel(TunnelSpecErrorType),
    ServerCannotCreateTunnels,
    ConnectPunchFoundDirectArgument(String),
    ConnectDirectFoundPunchArgument(String),
    ClientFoundServerArgument(String),
    ServerFoundClientArgument(String),
    MissingDestination,
    MissingTunnelSpecs,
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            Self::ConnectError(socket_error) => socket_error.fmt(f),
            Self::ListenError(socket_error) => socket_error.fmt(f),
            Self::MyIpError(ip_error) => ip_error.fmt(f),
            Self::LaneCount(lane_count_error) => lane_count_error.fmt(f),
            Self::PortStart(port_start_error) => port_start_error.fmt(f),
            Self::LocalTunnel(tunnel_spec_error) => tunnel_spec_error.fmt(f),
            Self::RemoteTunnel(tunnel_spec_error) => tunnel_spec_error.fmt(f),
            Self::ServerCannotCreateTunnels => write!(f, "Cannot create tunnels in server mode, only clients can create tunnels"),
            Self::ConnectDirectFoundPunchArgument(arg) => write!(
                f,
                "Previous arguments indicated using a direct connection, but {arg} is indicating a holepunched connection"
            ),
            Self::ConnectPunchFoundDirectArgument(arg) => write!(
                f,
                "Previous arguments indicated using a holepunched connection, but {arg} is indicating a direct connection"
            ),
            Self::ClientFoundServerArgument(arg) => {
                write!(f, "Previous arguments indicated client mode, but {arg} is indicating server mode")
            }
            Self::ServerFoundClientArgument(arg) => {
                write!(f, "Previous arguments indicated server mode, but {arg} is indicating client mode")
            }
            Self::MissingDestination => write!(f, "When running on client mode, a destination address must be specified"),
            Self::MissingTunnelSpecs => write!(f, "When running on client mode, you must specify at least one tunnel"),
        }
    }
}

struct StartupArgumentsParser {
    verbose: bool,
    silent: bool,
    connect_method: Option<ConnectMethod>,
    startup_mode: Option<StartupMode>,
}

impl StartupArgumentsParser {
    const fn new() -> Self {
        Self {
            verbose: false,
            silent: false,
            connect_method: None,
            startup_mode: None,
        }
    }

    fn ensure_startup_mode_client(&mut self, arg: String) -> Result<String, ArgumentsError> {
        match &mut self.startup_mode {
            None => {
                self.startup_mode = Some(StartupMode::Client(StartClientConfig::new()));
                Ok(arg)
            }
            Some(StartupMode::Client(_)) => Ok(arg),
            Some(StartupMode::Server(_)) => Err(ArgumentsError::ServerFoundClientArgument(arg)),
        }
    }

    fn ensure_startup_mode_server(&mut self, arg: String) -> Result<String, ArgumentsError> {
        match &mut self.startup_mode {
            None => {
                self.startup_mode = Some(StartupMode::Server(StartServerConfig::new()));
                Ok(arg)
            }
            Some(StartupMode::Server(_)) => Ok(arg),
            Some(StartupMode::Client(_)) => Err(ArgumentsError::ClientFoundServerArgument(arg)),
        }
    }

    fn modify_connect_method_direct<F>(&mut self, arg: String, f: F) -> Result<(), ArgumentsError>
    where
        F: FnOnce(String, &mut Vec<SocketAddr>) -> Result<(), ArgumentsError>,
    {
        match &mut self.connect_method {
            None => {
                let mut sockets = Vec::new();
                f(arg, &mut sockets)?;
                self.connect_method = Some(ConnectMethod::Direct(sockets));
            }
            Some(ConnectMethod::Direct(sockets)) => f(arg, sockets)?,
            Some(ConnectMethod::Punch(_)) => return Err(ArgumentsError::ConnectPunchFoundDirectArgument(arg)),
        }

        Ok(())
    }

    fn modify_connect_method_punch<F>(&mut self, arg: String, f: F) -> Result<(), ArgumentsError>
    where
        F: FnOnce(String, &mut PunchConfig) -> Result<(), ArgumentsError>,
    {
        match &mut self.connect_method {
            None => {
                let mut punch_config = PunchConfig::new();
                f(arg, &mut punch_config)?;
                self.connect_method = Some(ConnectMethod::Punch(punch_config));
            }
            Some(ConnectMethod::Punch(punch_config)) => f(arg, punch_config)?,
            Some(ConnectMethod::Direct(_)) => return Err(ArgumentsError::ConnectDirectFoundPunchArgument(arg)),
        }

        Ok(())
    }

    fn modify_startup_mode_client<F>(&mut self, arg: String, is_tunnel: bool, f: F) -> Result<(), ArgumentsError>
    where
        F: FnOnce(String, &mut StartClientConfig) -> Result<(), ArgumentsError>,
    {
        match &mut self.startup_mode {
            None => {
                let mut client_config = StartClientConfig::new();
                f(arg, &mut client_config)?;
                self.startup_mode = Some(StartupMode::Client(client_config));
            }
            Some(StartupMode::Client(client_config)) => f(arg, client_config)?,
            Some(StartupMode::Server(_)) if is_tunnel => return Err(ArgumentsError::ServerCannotCreateTunnels),
            Some(StartupMode::Server(_)) => return Err(ArgumentsError::ServerFoundClientArgument(arg)),
        }

        Ok(())
    }

    /*fn modify_startup_mode_server<F>(&mut self, arg: String, f: F) -> Result<(), ArgumentsError>
    where
        F: FnOnce(String, &mut StartServerConfig) -> Result<(), ArgumentsError>,
    {
        match &mut self.startup_mode {
            None => {
                let mut server_config = StartServerConfig::new();
                f(arg, &mut server_config)?;
                self.startup_mode = Some(StartupMode::Server(server_config));
            }
            Some(StartupMode::Server(server_config)) => f(arg, server_config)?,
            Some(StartupMode::Client(_)) => return Err(ArgumentsError::ClientFoundServerArgument(arg)),
        }

        Ok(())
    }*/

    fn complete(self) -> Result<StartupArguments, ArgumentsError> {
        let startup_mode = self.startup_mode.unwrap_or_else(|| StartupMode::Server(StartServerConfig::new()));
        let mut connect_method = self.connect_method.unwrap_or_else(|| ConnectMethod::Direct(Vec::new()));

        if let ConnectMethod::Direct(sockets) = &mut connect_method {
            if sockets.is_empty() {
                if startup_mode.is_client() {
                    return Err(ArgumentsError::MissingDestination);
                }

                sockets.push(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), DEFAULT_PORT));
                sockets.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_PORT));
            }
        }

        if let StartupMode::Client(client_config) = &startup_mode {
            if client_config.tunnels.is_empty() {
                return Err(ArgumentsError::MissingTunnelSpecs);
            }
        }

        Ok(StartupArguments::new(self.verbose, self.silent, connect_method, startup_mode))
    }
}

fn try_parse_general_argument(result: &mut StartupArgumentsParser, maybe_arg: &mut Option<String>) -> Result<bool, ArgumentsError> {
    let arg = match maybe_arg.take() {
        Some(s) => s,
        None => return Ok(false),
    };

    if arg.eq("-v") || arg.eq_ignore_ascii_case("--verbose") {
        result.verbose = true;
    } else if arg.eq("-s") || arg.eq_ignore_ascii_case("--silent") {
        result.silent = true;
    } else {
        *maybe_arg = Some(arg);
    }

    Ok(maybe_arg.is_none())
}

fn try_parse_client_argument<F>(
    result: &mut StartupArgumentsParser,
    maybe_arg: &mut Option<String>,
    get_next_arg: F,
) -> Result<bool, ArgumentsError>
where
    F: FnOnce() -> Option<String>,
{
    let arg = match maybe_arg.take() {
        Some(s) => s,
        None => return Ok(false),
    };

    if arg.eq("--client") {
        result.ensure_startup_mode_client(arg)?;
    } else if arg.eq("--connect") {
        let arg = result.ensure_startup_mode_client(arg)?;
        result.modify_connect_method_direct(arg, |arg, sockets| {
            parse_socket_arg(sockets, arg, get_next_arg(), DEFAULT_PORT).map_err(ArgumentsError::ConnectError)
        })?;
    } else {
        *maybe_arg = Some(arg);
    }

    Ok(maybe_arg.is_none())
}

fn try_parse_server_argument<F>(
    result: &mut StartupArgumentsParser,
    maybe_arg: &mut Option<String>,
    get_next_arg: F,
) -> Result<bool, ArgumentsError>
where
    F: FnOnce() -> Option<String>,
{
    let arg = match maybe_arg.take() {
        Some(s) => s,
        None => return Ok(false),
    };

    if arg.eq("--server") {
        result.ensure_startup_mode_server(arg)?;
    } else if arg.eq("--listen") {
        let arg = result.ensure_startup_mode_server(arg)?;
        result.modify_connect_method_direct(arg, |arg, sockets| {
            parse_socket_arg(sockets, arg, get_next_arg(), DEFAULT_PORT).map_err(ArgumentsError::ListenError)
        })?;
    } else {
        *maybe_arg = Some(arg);
    }

    Ok(maybe_arg.is_none())
}

fn try_parse_punch_argument<F>(
    result: &mut StartupArgumentsParser,
    maybe_arg: &mut Option<String>,
    get_next_arg: F,
) -> Result<bool, ArgumentsError>
where
    F: FnOnce() -> Option<String>,
{
    let arg = match maybe_arg.take() {
        Some(s) => s,
        None => return Ok(false),
    };

    if arg.eq("--punch") {
        result.modify_connect_method_punch(arg, |_, _| Ok(()))?;
    } else if arg.eq_ignore_ascii_case("--my-ip") {
        result.modify_connect_method_punch(arg, |arg, punch_config| {
            punch_config.my_ip = Some(parse_ip_addr_arg(arg, get_next_arg()).map_err(ArgumentsError::MyIpError)?);
            Ok(())
        })?;
    } else if arg.eq_ignore_ascii_case("--lane-count") {
        result.modify_connect_method_punch(arg, |arg, punch_config| {
            punch_config.lane_count = parse_lane_count_arg(arg, get_next_arg())?;
            Ok(())
        })?;
    } else if arg.eq_ignore_ascii_case("--port-start") {
        result.modify_connect_method_punch(arg, |arg, punch_config| {
            punch_config.port_start = Some(parse_port_number_arg(arg, get_next_arg()).map_err(ArgumentsError::PortStart)?);
            Ok(())
        })?;
    } else {
        *maybe_arg = Some(arg);
    }

    Ok(maybe_arg.is_none())
}

fn try_parse_tunnel_argument<F>(
    result: &mut StartupArgumentsParser,
    maybe_arg: &mut Option<String>,
    get_next_arg: F,
) -> Result<bool, ArgumentsError>
where
    F: FnOnce() -> Option<String>,
{
    let arg = match maybe_arg.take() {
        Some(s) => s,
        None => return Ok(false),
    };

    if arg.starts_with("-L") {
        result.modify_startup_mode_client(arg, true, |arg, client_config| {
            let idx = client_config.tunnels.len();
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Local, arg, 2, idx, get_next_arg);
            client_config.tunnels.push(spec_result.map_err(ArgumentsError::LocalTunnel)?);
            Ok(())
        })?;
    } else if arg.eq("--local-tunnel") {
        result.modify_startup_mode_client(arg, true, |arg, client_config| {
            let idx = client_config.tunnels.len();
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Local, arg, 14, idx, get_next_arg);
            client_config.tunnels.push(spec_result.map_err(ArgumentsError::LocalTunnel)?);
            Ok(())
        })?;
    } else if arg.starts_with("-R") {
        result.modify_startup_mode_client(arg, true, |arg, client_config| {
            let idx = client_config.tunnels.len();
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Remote, arg, 2, idx, get_next_arg);
            client_config.tunnels.push(spec_result.map_err(ArgumentsError::RemoteTunnel)?);
            Ok(())
        })?;
    } else if arg.eq("--remote-tunnel") {
        result.modify_startup_mode_client(arg, true, |arg, client_config| {
            let idx = client_config.tunnels.len();
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Remote, arg, 15, idx, get_next_arg);
            client_config.tunnels.push(spec_result.map_err(ArgumentsError::RemoteTunnel)?);
            Ok(())
        })?;
    } else {
        *maybe_arg = Some(arg);
    }

    Ok(maybe_arg.is_none())
}

pub fn parse_arguments<T>(mut args: T) -> Result<ArgumentsRequest, ArgumentsError>
where
    T: Iterator<Item = String>,
{
    let mut result = StartupArgumentsParser::new();

    // Ignore the first argument, as it's by convention the name of the program
    args.next();

    while let Some(arg) = args.next() {
        if arg.is_empty() {
            continue;
        } else if arg.eq("-h") || arg.eq_ignore_ascii_case("--help") {
            return Ok(ArgumentsRequest::Help);
        } else if arg.eq("-V") || arg.eq_ignore_ascii_case("--version") {
            return Ok(ArgumentsRequest::Version);
        }

        let mut maybe_arg = Some(arg);
        let _ = !try_parse_general_argument(&mut result, &mut maybe_arg)?
            && !try_parse_client_argument(&mut result, &mut maybe_arg, || args.next())?
            && !try_parse_server_argument(&mut result, &mut maybe_arg, || args.next())?
            && !try_parse_punch_argument(&mut result, &mut maybe_arg, || args.next())?
            && !try_parse_tunnel_argument(&mut result, &mut maybe_arg, || args.next())?;

        if let Some(arg) = maybe_arg {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    let result = result.complete()?;
    Ok(ArgumentsRequest::Run(result))
}
