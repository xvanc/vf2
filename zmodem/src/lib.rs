#![no_std]
#![feature(slice_flatten)]

#[cfg(feature = "std")]
extern crate std;

macro_rules! println {
    ($(,)?) => { $crate::print(format_args!("\n")) };
    ($($t:tt)*) => { $crate::print(format_args!("{}\n", format_args!($($t)*))) };
}

#[cfg(feature = "std")]
fn print(args: core::fmt::Arguments) {
    use std::io::Write;

    let mut stdout = std::io::stdout();
    stdout.write_fmt(args).ok();
    stdout.flush().ok();
}

#[cfg(not(feature = "std"))]
fn print(_args: core::fmt::Arguments) {}

pub mod proto;
pub mod recv;

pub use recv::receive;

use core::{fmt, time::Duration};
use proto::{FrameEncoding, FrameHeader};

pub trait SerialDevice {
    type Error: fmt::Debug;

    /// Transmit a byte on the serial device
    fn send(&mut self, byte: u8) -> Result<(), Self::Error>;

    /// Receive a byte on the serial device
    ///
    /// Returns `Some` if a byte is received before the timeout expires, `None` otherwise,
    /// or `Err` if an error occurs.
    fn recv(&mut self, timeout: Duration) -> Result<Option<u8>, Self::Error>;
}

#[derive(Debug)]
pub enum Error<D> {
    InvalidFrameEncoding(FrameEncoding),
    UnexpectedFrame(FrameHeader),
    InvalidHex(u8),
    InvalidEscape(u8),
    TimedOut,
    Device(D),
}

// Interal wrapper around `SerialDevice` to translate `None` into our `Error::TimedOut`.
struct Device<D: SerialDevice> {
    dev: D,
}

impl<D: SerialDevice> Device<D> {
    fn send(&mut self, byte: u8) -> Result<(), Error<D::Error>> {
        // println!("tx: {byte:02x} ({:?})", byte as char);
        self.dev.send(byte).map_err(Error::Device)
    }

    #[allow(clippy::let_and_return)]
    fn recv(&mut self, timeout: Duration) -> Result<u8, Error<D::Error>> {
        let result = match self.dev.recv(timeout) {
            Ok(Some(byte)) => Ok(byte),
            Ok(None) => Err(Error::TimedOut),
            Err(error) => Err(Error::Device(error)),
        };
        // match &result {
        //     Ok(byte) => println!("rx: {byte:02x} ({:?})", *byte as char),
        //     Err(error) => println!("rx: {error:?}"),
        // }
        result
    }
}

const TIMEOUT_DURATION: Duration = Duration::from_secs(600);

pub fn crc16(buf: &[u8]) -> u16 {
    crc::Crc::<u16>::new(&crc::CRC_16_XMODEM).checksum(buf)
}

pub struct FromHexError(u8);

impl<D> From<FromHexError> for Error<D> {
    fn from(error: FromHexError) -> Self {
        Error::InvalidHex(error.0)
    }
}

fn from_hex(bytes: [u8; 2]) -> Result<u8, FromHexError> {
    fn from_hex_nibble(nibble: u8) -> Result<u8, FromHexError> {
        match nibble {
            b'0'..=b'9' => Ok(nibble - b'0'),
            b'a'..=b'f' => Ok(10 + (nibble - b'a')),
            _ => Err(FromHexError(nibble)),
        }
    }

    Ok(from_hex_nibble(bytes[0])? << 4 | from_hex_nibble(bytes[1])?)
}

fn to_hex(byte: u8) -> [u8; 2] {
    fn to_hex_nibble(nibble: u8) -> u8 {
        if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + (nibble - 10)
        }
    }
    [to_hex_nibble(byte >> 4), to_hex_nibble(byte & 0xf)]
}
