use std::{
    io::{self, Error, ErrorKind},
    net::Ipv4Addr,
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

const IPV4_PARSE_ERROR: &str = "Couldn't find public IP: Server responded with invalid IPv4 address";

/// Gets this machine's publicly-visible IPv4 address with a request to `api.ipify.org`.
pub async fn get_public_ipv4() -> io::Result<Ipv4Addr> {
    let iter = tokio::net::lookup_host("api.ipify.org:80").await?;
    let iter = iter.filter(|addr| addr.is_ipv4());

    let mut last_error = None;
    let mut stream = None;
    for remote_addr in iter {
        let socket = tokio::net::TcpSocket::new_v4()?;
        let connect_result = socket.connect(remote_addr).await;
        match connect_result {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(error) => {
                last_error = Some(error);
            }
        }
    }

    let mut stream = match (stream, last_error) {
        (None, None) => {
            return Err(Error::new(
                ErrorKind::Other,
                "Couldn't connect to api.ipify.org. Are you connected to the internet?",
            ))
        }
        (None, Some(last_error)) => return Err(last_error),
        (Some(s), _) => s,
    };

    let (mut read_half, mut write_half) = stream.split();
    write_half.write_all(b"GET / HTTP/1.1\r\nHost: api.ipify.org\r\n\r\n").await?;

    let mut buf = [0u8; 1024];
    let mut buf_len = 0;
    let mut newline_counter = 0;

    loop {
        let bytes_read = match read_half.read(&mut buf[buf_len..]).await {
            Ok(0) => break,
            Ok(v) => v,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        if newline_counter == 2 {
            buf_len += bytes_read;
        } else {
            for i in 0..bytes_read {
                let b = buf[i];

                if b == b'\r' {
                    continue;
                } else if b != b'\n' {
                    newline_counter = 0;
                } else {
                    newline_counter += 1;
                    if newline_counter == 2 {
                        write_half.shutdown().await?;
                        buf.copy_within((i + 1)..bytes_read, 0);
                        buf_len = bytes_read - (i + 1);
                        break;
                    }
                }
            }
        }
    }

    buf[..buf_len].iter_mut().filter(|b| !b.is_ascii_graphic()).for_each(|b| *b = b'?');

    // SAFETY: We replaced all non-ascii-graphic chars with b'?', which ensures buf[..buf_len] is valid UTF-8
    let s = unsafe { std::str::from_utf8_unchecked(&buf[..buf_len]) };

    s.parse::<Ipv4Addr>().map_err(|_| {
        let message = format!("{IPV4_PARSE_ERROR}: {s}");
        Error::new(ErrorKind::InvalidData, message)
    })
}
