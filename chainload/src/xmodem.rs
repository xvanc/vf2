use core::time::Duration;
use bytemuck::Zeroable;
use crate::uart::{Uart, UartError};

pub const START_OF_HEADER: u8 = 0x01;
pub const END_OF_TRANSMISSION: u8 = 0x04;
pub const ACK: u8 = 0x06;
pub const NAK: u8 = 0x15;
pub const END_OF_TRANSMISSION_BLOCK: u8 = 0x17;
// pub const CANCEL: u8 = 0x18;
pub const CHECKSUM_REQUEST: u8 = b'C';

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Packet {
    r#type: u8,
    id: u8,
    id_inverted: u8,
    data: [u8; 128],
    crc: [u8; 2],
}

#[derive(Debug)]
pub enum Error {
    BadPacketId,
    BadPacketType(u8),
    Uart(UartError),
    NoResponse,
}

impl From<UartError> for Error {
    fn from(error: UartError) -> Self {
        Self::Uart(error)
    }
}

pub struct Receiver<'uart> {
    buffer: Packet,
    uart: &'uart mut Uart,
}

impl<'uart> Receiver<'uart> {
    pub fn new(uart: &'uart mut Uart) -> Receiver<'uart> {
        Self {
            buffer: Packet::zeroed(),
            uart,
        }
    }

    pub fn receive(&mut self, mut f: impl FnMut(&[u8; 128])) -> Result<(), Error> {
        self.uart.transmit(CHECKSUM_REQUEST)?;
        let mut id = None;

        loop {
            let bytes = &mut bytemuck::bytes_of_mut(&mut self.buffer)[1..];
            match self.uart.receive_timeout(Duration::from_millis(500)) {
                Ok(byte) => match byte {
                    START_OF_HEADER => {}
                    END_OF_TRANSMISSION => {
                        self.uart.transmit(ACK)?;
                        break;
                    }
                    ty => return Err(Error::BadPacketType(ty)),
                }
                Err(UartError::TimedOut) => return Err(Error::NoResponse),
            }

            for byte in bytes {
                *byte = self.uart.receive()?;
            }

            let checksum_good = checksum(&self.buffer.data) == u16::from_be_bytes(self.buffer.crc);
            let id_good = self.buffer.id == !self.buffer.id_inverted;
            if !checksum_good || !id_good {
                self.uart.transmit(NAK)?;
                continue;
            }

            match id {
                None => id = Some(self.buffer.id),
                Some(id) => {
                    if !(id == self.buffer.id + 1 || id == self.buffer.id - 1) {
                        return Err(Error::BadPacketId);
                    }
                }
            }

            f(&self.buffer.data);
        }

        match self.uart.receive()? {
            END_OF_TRANSMISSION_BLOCK => {}
            ty => return Err(Error::BadPacketType(ty)),
        }
        self.uart.transmit(ACK)?;

        Ok(())
    }
}

pub fn checksum(bytes: &[u8]) -> u16 {
    let mut checksum = 0;

    for byte in bytes.iter().copied() {
        checksum ^= u16::from(byte) << 8;
        for _ in 0..8 {
            match (checksum as i16).is_negative() {
                true => checksum = (checksum << 1) ^ 0x1021,
                false => checksum <<= 1,
            }
        }
    }

    checksum
}

pub fn receive(uart: &mut Uart, load: *mut u8) -> Result<(), Error> {
    uart.flush_receiver()?;
    let mut receiver = Receiver::new(uart);
    let mut offset = 0;
    loop {
        let result = receiver.receive(|packet| unsafe {
            load.add(offset).copy_from(packet.as_ptr(), packet.len());
            offset += packet.len();
        });

        match result {
            Err(Error::NoResponse) => {
                continue;
            }
            Err(error) => return Err(error),
            Ok(()) => break,
        }
    }
    Ok(())
}
