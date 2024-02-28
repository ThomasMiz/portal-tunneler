use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    num::NonZeroU16,
    ops::BitXorAssign,
};

use base64::Engine;

use crate::utils::get_current_timestamp;

pub const CONNECTION_CODE_MAX_LENGTH_BYTES: usize = 17 + 2 + 2 + 8 + 2;
pub const CONNECTION_STRING_MAX_LENGTH_CHARS: usize = (CONNECTION_CODE_MAX_LENGTH_BYTES * 4 + 2) / 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionCode {
    pub address: IpAddr,
    pub port_start: u16,
    pub lane_count: NonZeroU16,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeserializeError {
    InvalidBase64,
    UnexpectedEnd,
    InvalidIpTypeByte,
    ZeroLaneCount,
    OverflowingLaneCount,
    BadChecksum,
    TooLong,
}

fn calc_checksum(buf: &[u8]) -> u16 {
    let mut ones_count = 0u8;
    let mut xored = 0x69;

    for ele in buf {
        ones_count = ones_count.wrapping_add(ele.count_ones() as u8);
        xored.bitxor_assign(*ele);
    }

    (xored as u16) | ((ones_count as u16) << 8)
}

impl ConnectionCode {
    pub fn new(address: IpAddr, port_start: u16, lane_count: NonZeroU16) -> Self {
        Self {
            address,
            port_start,
            lane_count,
            timestamp: get_current_timestamp(),
        }
    }

    pub fn serialize_to_bytes(&self, buf: &mut [u8]) -> usize {
        let mut index;

        match self.address {
            IpAddr::V4(ipv4) => {
                buf[0] = 4;
                buf[1..5].copy_from_slice(&ipv4.octets());
                index = 5;
            }
            IpAddr::V6(ipv6) => {
                buf[0] = 6;
                buf[1..17].copy_from_slice(&ipv6.octets());
                index = 17;
            }
        }

        buf[index..(index + 2)].copy_from_slice(&self.port_start.to_le_bytes());
        index += 2;

        buf[index..(index + 2)].copy_from_slice(&self.lane_count.get().to_le_bytes());
        index += 2;

        buf[index..(index + 8)].copy_from_slice(&self.timestamp.to_le_bytes());
        index += 8;

        let checksum = calc_checksum(&buf[..index]);
        buf[index..(index + 2)].copy_from_slice(&checksum.to_le_bytes());
        index += 2;

        index
    }

    pub fn serialize_to_string(&self) -> String {
        let mut s = String::with_capacity(CONNECTION_STRING_MAX_LENGTH_CHARS);
        let mut buf = [0u8; CONNECTION_CODE_MAX_LENGTH_BYTES];
        let len = self.serialize_to_bytes(&mut buf);
        base64::prelude::BASE64_URL_SAFE_NO_PAD.encode_string(&buf[..len], &mut s);
        s
    }

    pub fn deserialize_from_bytes(buf: &[u8]) -> Result<ConnectionCode, DeserializeError> {
        fn check_buf_len(buf: &[u8], min_len: usize) -> Result<(), DeserializeError> {
            match buf.len() >= min_len {
                true => Ok(()),
                false => Err(DeserializeError::UnexpectedEnd),
            }
        }

        check_buf_len(buf, 0)?;
        let mut index;

        let address = match buf[0] {
            4 => {
                check_buf_len(buf, 5)?;
                let mut octets = [0u8; 4];
                octets.copy_from_slice(&buf[1..5]);
                index = 5;
                IpAddr::V4(Ipv4Addr::from(octets))
            }
            6 => {
                check_buf_len(buf, 17)?;
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&buf[1..17]);
                index = 17;
                IpAddr::V6(Ipv6Addr::from(octets))
            }
            _ => return Err(DeserializeError::InvalidIpTypeByte),
        };

        check_buf_len(buf, index + 2)?;
        let port_start = u16::from_le_bytes([buf[index], buf[index + 1]]);
        index += 2;

        check_buf_len(buf, index + 2)?;
        let lane_count_u16 = u16::from_le_bytes([buf[index], buf[index + 1]]);
        let lane_count = NonZeroU16::new(lane_count_u16).ok_or(DeserializeError::ZeroLaneCount)?;
        index += 2;

        let (_, overflows) = port_start.overflowing_add(lane_count.get());
        if overflows {
            return Err(DeserializeError::OverflowingLaneCount);
        }

        check_buf_len(buf, index + 8)?;
        let mut timestamp_bytes = [0u8; 8];
        timestamp_bytes.copy_from_slice(&buf[index..(index + 8)]);
        let timestamp = u64::from_le_bytes(timestamp_bytes);
        index += 8;

        check_buf_len(buf, index + 2)?;
        let checksum = u16::from_le_bytes([buf[index], buf[index + 1]]);
        if checksum != calc_checksum(&buf[..index]) {
            return Err(DeserializeError::BadChecksum);
        }
        index += 2;

        if buf.len() != index {
            return Err(DeserializeError::TooLong);
        }

        Ok(Self {
            address,
            port_start,
            lane_count,
            timestamp,
        })
    }

    pub fn deserialize_from_str(string: &str) -> Result<ConnectionCode, DeserializeError> {
        let mut buf = [0u8; CONNECTION_CODE_MAX_LENGTH_BYTES + 2];
        let buf_len = match base64::prelude::BASE64_URL_SAFE_NO_PAD.decode_slice(string, &mut buf) {
            Ok(v) => v,
            Err(base64::DecodeSliceError::OutputSliceTooSmall) => return Err(DeserializeError::TooLong),
            Err(base64::DecodeSliceError::DecodeError(_)) => return Err(DeserializeError::InvalidBase64),
        };

        Self::deserialize_from_bytes(&buf[..buf_len])
    }
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, num::NonZeroU16};

    use crate::puncher::connection_code::DeserializeError;

    use super::ConnectionCode;

    #[test]
    fn test1() {
        let addresses: [IpAddr; 5] = [
            IpAddr::V4("69.22.4.0".parse().unwrap()),
            IpAddr::V4("1.2.3.4".parse().unwrap()),
            IpAddr::V4("123.210.123.210".parse().unwrap()),
            IpAddr::V6("::1".parse().unwrap()),
            IpAddr::V6("1234::9c9:3ab2:f332:23ec".parse().unwrap()),
        ];

        let port_starts = [
            0, 500, 1024, 1920, 5000, 7912, 1250, 1251, 1252, 3434, 9090, 12312, 32132, 48912, 65535,
        ];

        let lane_counts = [1, 2, 3, 4, 5, 6, 7, 8, 10, 50, 65535];

        for address in addresses {
            for port_start in port_starts {
                for lane_count in lane_counts {
                    let code = ConnectionCode::new(address, port_start, NonZeroU16::new(lane_count).unwrap());
                    let s = code.serialize_to_string();
                    let deserialized = ConnectionCode::deserialize_from_str(&s);

                    assert_eq!(Ok(code), deserialized);
                }
            }
        }
    }

    #[test]
    fn test_bad_checksum() {
        let code = ConnectionCode::new("69.22.4.0".parse().unwrap(), 43434, NonZeroU16::new(69).unwrap());
        let mut s = code.serialize_to_string();
        let last = s.pop().unwrap();
        let lastlast = s.pop().unwrap();
        s.push(match lastlast {
            'a' => 'b',
            _ => 'a',
        });
        s.push(last);

        let result = ConnectionCode::deserialize_from_str(&s);
        assert_eq!(result, Err(DeserializeError::BadChecksum))
    }
}
