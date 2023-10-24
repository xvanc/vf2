use crate::{
    crc16, crc32, from_hex,
    proto::{consts::*, FrameEncoding, FrameHeader, FrameType, PacketType},
    to_hex, Device, Error, SerialDevice, TIMEOUT_DURATION,
};
use bytemuck::{Pod, Zeroable};
use core::time::Duration;

pub struct Receiver<D: SerialDevice> {
    dev: Device<D>,
}

impl<D: SerialDevice> Receiver<D> {
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

    pub fn send_frame(&mut self, frame: FrameHeader) -> Result<(), Error<D::Error>> {
        let buf = bytemuck::bytes_of(&frame);
        let crc = crc16(&buf[1..], None);

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
}

#[derive(Clone, Copy, Debug)]
enum RecvEnc {
    Hex,
    Esc,
    Raw,
}

impl<D: SerialDevice> Receiver<D> {
    pub fn new(dev: D) -> Receiver<D> {
        Self {
            dev: Device { dev },
        }
    }

    /// Receive a raw byte (no unescaping or hex-decoding)
    fn recv_raw(&mut self, timeout: Duration) -> Result<u8, Error<D::Error>> {
        self.dev.recv(timeout)
    }

    /// Receive an unescaped byte.
    fn recv_esc(&mut self, timeout: Duration) -> Result<u8, Error<D::Error>> {
        let byte = match self.dev.recv(timeout)? {
            ZDLE => match self.dev.recv(timeout)? {
                byte if byte & 0x60 == 0x40 => byte ^ 0x40,
                ZRUB0 => 0x7f,
                ZRUB1 => 0xff,
                byte => panic!("unhandled ZDLE escape: {byte:02x}"),
            },
            byte => byte,
        };
        Ok(byte)
    }

    /// Receive a hex-encoded byte
    fn recv_hex(&mut self, timeout: Duration) -> Result<u8, Error<D::Error>> {
        // NOTE: Hex-encoding ignores parity.
        let hi = self.dev.recv(timeout)? & 0x7f;
        let lo = self.dev.recv(timeout)? & 0x7f;
        Ok(from_hex([hi, lo])?)
    }

    /// Receive a byte, decoded according to `enc`
    fn recv_byte(&mut self, enc: RecvEnc, timeout: Duration) -> Result<u8, Error<D::Error>> {
        match enc {
            RecvEnc::Hex => self.recv_hex(timeout),
            RecvEnc::Esc => self.recv_esc(timeout),
            RecvEnc::Raw => self.recv_raw(timeout),
        }
    }

    fn recv<T: Pod + Zeroable>(
        &mut self,
        enc: RecvEnc,
        timeout: Duration,
    ) -> Result<T, Error<D::Error>> {
        let mut buf = T::zeroed();
        for byte in bytemuck::bytes_of_mut(&mut buf) {
            *byte = self.recv_byte(enc, timeout)?;
        }
        Ok(buf)
    }

    /// Receive the data portion of a packet.
    ///
    /// Returns `None` if the entire buffer is filled, otherwise returns the packet type along
    /// with the number of bytes written to the buffer.
    fn receive_data(&mut self, buf: &mut [u8]) -> Result<Option<(u8, usize)>, Error<D::Error>> {
        let mut i = 0;
        while i < buf.len() {
            let byte = match self.dev.recv(TIMEOUT_DURATION)? {
                ZDLE => match self.dev.recv(TIMEOUT_DURATION)? {
                    byte if byte & 0x60 == 0x40 => byte ^ 0x40,
                    ZRUB0 => 0x7f,
                    ZRUB1 => 0xff,
                    packet_type @ (ZCRCE | ZCRCG | ZCRCQ | ZCRCW) => {
                        return Ok(Some((packet_type, i)));
                    }
                    byte => panic!("unhandled ZDLE escape: {byte:02x}"),
                },
                byte => byte,
            };
            buf[i] = byte;
            i += 1;
        }
        Ok(None)
    }

    pub fn receive_data_packet<'buf>(
        &mut self,
        encoding: FrameEncoding,
        buf: &'buf mut [u8],
    ) -> Result<(PacketType, &'buf [u8]), Error<D::Error>> {
        let (packet_type, len) = match self.receive_data(&mut buf[..1024])? {
            Some((packet_type, len)) => (packet_type, len),
            None => {
                let byte = self.dev.recv(TIMEOUT_DURATION)?;
                assert!(byte == ZDLE, "{byte:02x}");
                (self.dev.recv(TIMEOUT_DURATION)?, 1024)
            }
        };
        let packet_type = PacketType(packet_type);

        let (crc, our_crc) = if encoding == FrameEncoding::BIN32 {
            let crc = u32::from_be_bytes(self.recv(RecvEnc::Esc, TIMEOUT_DURATION)?);
            let our_crc = crc32(&buf[..len], Some(packet_type.0));
            (crc, our_crc)
        } else {
            let crc = u16::from_be_bytes(self.recv(RecvEnc::Esc, TIMEOUT_DURATION)?) as u32;
            let our_crc = crc16(&buf[..len], Some(packet_type.0)) as u32;
            (crc, our_crc)
        };

        if crc != our_crc {
            println!("packet crc error");
        }

        // println!("data: {len} bytes, {packet_type:?}, crc {crc:#x}",);

        Ok((packet_type, &buf[..len]))
    }

    pub fn receive_frame_header(
        &mut self,
        timeout: Duration,
    ) -> Result<FrameHeader, Error<D::Error>> {
        let mut prec_zpad = false;
        loop {
            match self.recv_raw(timeout)? {
                ZPAD => match self.recv_raw(timeout)? {
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

        let encoding = FrameEncoding(self.recv_raw(TIMEOUT_DURATION)?);
        if !matches!(
            encoding,
            FrameEncoding::HEX | FrameEncoding::BIN16 | FrameEncoding::BIN32
        ) {
            return Err(Error::InvalidFrameEncoding(encoding));
        }

        let enc = match encoding {
            FrameEncoding::HEX => {
                if !prec_zpad {
                    println!("hex frame with single zpad");
                }
                RecvEnc::Hex
            }
            _ => RecvEnc::Esc,
        };

        let frame_type = FrameType(self.recv(enc, TIMEOUT_DURATION)?);
        let data = self.recv(enc, TIMEOUT_DURATION)?;

        let frame = FrameHeader {
            encoding,
            r#type: frame_type,
            data,
        };

        let (crc, our_crc) = if encoding == FrameEncoding::BIN32 {
            let crc = u32::from_be_bytes(self.recv(enc, TIMEOUT_DURATION)?);
            let our_crc = crc32(&bytemuck::bytes_of(&frame)[1..], None);
            (crc, our_crc)
        } else {
            let crc = u16::from_be_bytes(self.recv(enc, TIMEOUT_DURATION)?) as u32;
            let our_crc = crc16(&bytemuck::bytes_of(&frame)[1..], None) as u32;
            (crc, our_crc)
        };

        if crc != our_crc {
            println!("frame crc error");
        }

        if encoding == FrameEncoding::HEX {
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

        // println!("rx frame: {encoding:?}, {frame_type:?}, {data:02x?}");

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
            let frame = match receiver.receive_frame_header(timeout) {
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
            let frame = receiver.receive_frame_header(TIMEOUT_DURATION)?;
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
    assert!(receiver.recv::<u8>(RecvEnc::Raw, TIMEOUT_DURATION)? == b'O');
    assert!(receiver.recv::<u8>(RecvEnc::Raw, TIMEOUT_DURATION)? == b'O');

    Ok(output_offset)
}
