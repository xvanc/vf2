use crate::{
    crc16, from_hex,
    proto::{consts::*, FrameEncoding},
    proto::{FrameHeader, FrameType, PacketType},
    to_hex, Device, Error, SerialDevice, TIMEOUT_DURATION,
};
use bytemuck::{Pod, Zeroable};
use core::time::Duration;

pub struct Receiver<D: SerialDevice> {
    dev: Device<D>,
}

impl<D: SerialDevice> Receiver<D> {
    pub fn new(dev: D) -> Receiver<D> {
        Self {
            dev: Device { dev },
        }
    }

    fn send_hex(&mut self, byte: u8) -> Result<(), Error<D::Error>> {
        let bytes = to_hex(byte);
        self.dev.send(bytes[0])?;
        self.dev.send(bytes[1])?;
        Ok(())
    }

    fn send(&mut self, byte: u8, hex: bool) -> Result<(), Error<D::Error>> {
        if hex {
            self.send_hex(byte)
        } else {
            self.dev.send(byte)
        }
    }

    fn recv_hex(&mut self) -> Result<u8, Error<D::Error>> {
        Ok(from_hex([
            self.dev.recv(TIMEOUT_DURATION)?,
            self.dev.recv(TIMEOUT_DURATION)?,
        ])?)
    }

    fn recv(&mut self, hex: bool) -> Result<u8, Error<D::Error>> {
        if hex {
            self.recv_hex()
        } else {
            self.dev.recv(TIMEOUT_DURATION)
        }
    }

    fn recv_any<T: Pod + Zeroable>(&mut self) -> Result<T, Error<D::Error>> {
        let mut buf = T::zeroed();
        for byte in bytemuck::bytes_of_mut(&mut buf) {
            *byte = self.recv(false)?;
        }
        Ok(buf)
    }

    pub fn send_frame(&mut self, frame: FrameHeader) -> Result<(), Error<D::Error>> {
        let buf = bytemuck::bytes_of(&frame);
        let crc = crc16(&buf[1..]);

        // println!(
        //     "tx frame: {:?}, {:?}, {:02x?}",
        //     frame.encoding, frame.r#type, frame.data
        // );

        self.send(ZPAD, false)?;
        self.send(ZPAD, false)?;
        self.send(ZDLE, false)?;
        self.send(buf[0], false)?;
        for byte in &buf[1..] {
            self.send(*byte, frame.encoding == FrameEncoding::HEX)?;
        }
        for byte in crc.to_be_bytes() {
            self.send(byte, frame.encoding == FrameEncoding::HEX)?;
        }
        self.send(CR, false)?;
        self.send(0x80 | LF, false)?;
        self.send(XON, false)?;

        Ok(())
    }

    pub fn send_zrinit(&mut self) -> Result<(), Error<D::Error>> {
        self.send_frame(FrameHeader::new(FrameEncoding::HEX, FrameType::ZRINIT))
    }

    pub fn send_zrpos(&mut self, pos: u32) -> Result<(), Error<D::Error>> {
        self.send_frame(FrameHeader::new(FrameEncoding::HEX, FrameType::ZRPOS).set_count(pos))
    }

    pub fn send_zfin(&mut self) -> Result<(), Error<D::Error>> {
        self.send_frame(FrameHeader::new(FrameEncoding::HEX, FrameType::ZFIN))
    }

    pub fn receive_data_packet<'buf>(
        &mut self,
        encoding: FrameEncoding,
        buf: &'buf mut [u8],
    ) -> Result<(PacketType, &'buf [u8]), Error<D::Error>> {
        let mut i = 0;
        let mut push = |byte| {
            buf[i] = byte;
            i += 1;
        };

        let packet_type = loop {
            let byte = self.dev.recv(TIMEOUT_DURATION)?;
            match byte {
                ZDLE => match self.dev.recv(TIMEOUT_DURATION)? {
                    packet_type @ (ZCRCE | ZCRCG | ZCRCQ | ZCRCW) => break packet_type,
                    ZRUB0 => push(0x7f),
                    ZRUB1 => push(0xff),
                    byte if byte & 0x60 == 0x40 => push(byte ^ 0x40),
                    byte => push(byte),
                },
                byte => push(byte),
            }
        };

        let crc = if encoding == FrameEncoding::BIN16 {
            self.recv_any::<u16>()?.swap_bytes() as u32
        } else {
            self.recv_any::<u32>()?.swap_bytes()
        };

        // println!("data: {i} bytes, {packet_type:?}, crc {crc:#x}",);

        Ok((PacketType(packet_type), &buf[..i]))
    }

    pub fn receive_frame_header(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<FrameHeader, Error<D::Error>> {
        let mut prec_zpad = false;
        loop {
            match self.dev.recv(timeout.unwrap_or(TIMEOUT_DURATION))? {
                ZPAD => match self.dev.recv(timeout.unwrap_or(TIMEOUT_DURATION))? {
                    ZDLE => break,
                    byte => {
                        prec_zpad = byte == ZPAD;
                        continue;
                    }
                },
                ZDLE if prec_zpad => break,
                _ => continue,
            }
        }

        let encoding = FrameEncoding(self.dev.recv(TIMEOUT_DURATION)?);
        if !matches!(
            encoding,
            FrameEncoding::HEX | FrameEncoding::BIN16 | FrameEncoding::BIN32
        ) {
            return Err(Error::InvalidFrameEncoding(encoding));
        }

        let hex = encoding == FrameEncoding::HEX;
        if hex && !prec_zpad {
            println!("hex frame with single zpad");
        }

        let frame_type = FrameType(self.recv(hex)?);

        let mut payload = [0; 4];
        for byte in &mut payload {
            *byte = self.recv(hex)?;
        }

        let crc = if encoding == FrameEncoding::BIN32 {
            let mut crc = [0; 4];
            for byte in &mut crc {
                *byte = self.recv(false)?;
            }
            u32::from_le_bytes(crc)
        } else {
            let mut crc = [0; 2];
            for byte in &mut crc {
                *byte = self.recv(hex)?;
            }
            u16::from_le_bytes(crc) as u32
        };

        if hex {
            if self.dev.recv(TIMEOUT_DURATION)? != CR {
                println!("missing CR on hex frame");
            }
            if self.dev.recv(TIMEOUT_DURATION)? & 0x7f != LF {
                println!("missing LF on hex frame");
            }
            if !matches!(frame_type, FrameType::ZACK | FrameType::ZFIN)
                && self.dev.recv(TIMEOUT_DURATION)? != XON
            {
                println!("missing XON on hex frame");
            }
        }

        let frame = FrameHeader {
            encoding,
            r#type: frame_type,
            data: payload,
        };

        // println!("rx frame: {encoding:?}, {frame_type:?}, {payload:02x?}");

        Ok(frame)
    }
}

pub fn receive<D: SerialDevice>(dev: D, output: &mut [u8]) -> Result<usize, Error<D::Error>> {
    let mut receiver = Receiver::new(dev);
    let mut buf = [0; 0x1000];
    let mut output_offset = 0;

    // Use a shorter timeout for the first iteration of the loop, so we advertise
    // our ZRINIT more frequently until a session is started.
    let mut timeout = Duration::from_millis(500);

    'main: loop {
        // Receive the ZFILE header.
        let zfile = loop {
            receiver.send_zrinit()?;
            let frame = match receiver.receive_frame_header(Some(timeout)) {
                Ok(frame) => frame,
                Err(Error::TimedOut) => continue,
                Err(error) => return Err(error),
            };
            match frame.r#type {
                // Sender is requesting our ZRINIT header.
                FrameType::ZRQINIT => continue,
                // Begin file transfer.
                FrameType::ZFILE => break frame,
                // Finish session.
                FrameType::ZFIN => break 'main,
                _ => return Err(Error::UnexpectedFrame(frame)),
            }
        };

        // Now that we've begun a session, following loops should wait the normal amount
        // of time before giving up.
        timeout = TIMEOUT_DURATION;

        // Receive the data subpacket containing the file metadata.
        let (packet_type, _meta) = receiver.receive_data_packet(zfile.encoding, &mut buf)?;
        if packet_type != PacketType::ZCRCW {
            panic!("expected ZCRCW packet, got {packet_type:?}");
        }
        // for slice in meta.split(|b| *b == 0) {
        //     if slice.is_empty() {
        //         continue;
        //     }
        //     println!("meta: {}", core::str::from_utf8(slice).unwrap());
        // }

        let mut pos = 0;
        receiver.send_zrpos(pos)?;
        loop {
            let frame = receiver.receive_frame_header(None)?;
            match frame.r#type {
                FrameType::ZDATA => (),
                FrameType::ZEOF => break,
                frame_type => panic!("unknown packet type: {frame_type:?}"),
            }

            loop {
                let (packet_type, buf) = receiver.receive_data_packet(frame.encoding, &mut buf)?;

                pos += buf.len() as u32;
                assert!(output_offset + buf.len() <= output.len());
                output[output_offset..][..buf.len()].copy_from_slice(buf);
                output_offset += buf.len();

                match packet_type {
                    PacketType::ZCRCG => continue,
                    PacketType::ZCRCE => break,
                    _ => panic!("unknown packet type: {packet_type:?}"),
                }
            }
        }
    }

    receiver.send_zfin()?;
    assert!(receiver.recv(false)? == b'O');
    assert!(receiver.recv(false)? == b'O');

    Ok(output_offset)
}
