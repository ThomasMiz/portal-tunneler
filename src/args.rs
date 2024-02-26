use core::fmt;

pub fn get_version_string() -> String {
    format!(
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), " ({} {})"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn get_help_string() -> &'static str {
    "I need somebody"
}

#[derive(Debug, PartialEq)]
pub enum ArgumentsRequest {
    Help,
    Version,
    Run(StartupArguments),
}

#[derive(Debug, PartialEq)]
pub struct StartupArguments {
    pub is_server: bool,
}

impl StartupArguments {
    pub fn empty() -> Self {
        Self { is_server: false }
    }

    pub fn fill_empty_fields_with_defaults(&mut self) {}
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsError {
    UnknownArgument(String),
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
        }
    }
}

pub fn parse_arguments<T>(mut args: T) -> Result<ArgumentsRequest, ArgumentsError>
where
    T: Iterator<Item = String>,
{
    let mut result = StartupArguments::empty();

    // Ignore the first argument, as it's by convention the name of the program
    args.next();

    while let Some(arg) = args.next() {
        if arg.is_empty() {
            continue;
        } else if arg.eq("--server") {
            result.is_server = true;
        } else {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    result.fill_empty_fields_with_defaults();
    Ok(ArgumentsRequest::Run(result))
}
