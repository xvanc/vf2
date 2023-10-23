use bytemuck::Zeroable;
use nix::sys::termios;
use std::{
    error,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::prelude::OpenOptionsExt,
};

pub const START_OF_HEADER: u8 = 0x01;
pub const END_OF_TRANSMISSION: u8 = 0x04;
pub const ACK: u8 = 0x06;
pub const NAK: u8 = 0x15;
// pub const END_OF_TRANSMISSION_BLOCK: u8 = 0x17;
pub const CANCEL: u8 = 0x18;
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

pub trait SerialDevice {
    type Error;
    fn recv(&mut self) -> Result<u8, Self::Error>;
    fn send(&mut self, c: u8) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum Error<S> {
    // BadPacketId,
    BadPacketType(u8),
    Canceled,
    Serial(S),
}

impl<S> From<S> for Error<S> {
    fn from(s: S) -> Self {
        Self::Serial(s)
    }
}

#[cfg(feature = "std")]
impl<S: core::fmt::Display + core::fmt::Debug> std::error::Error for Error<S> {}
impl<S: core::fmt::Display + core::fmt::Debug> core::fmt::Display for Error<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            // Self::BadPacketId => write!(f, "bad XMODEM packet ID"),
            Self::BadPacketType(n) => write!(f, "bad XMODEM packet type {:#x}", n),
            Self::Canceled => write!(f, "XMODEM transfer canceled"),
            Self::Serial(s) => write!(f, "serial error: {}", s),
        }
    }
}

pub struct Sender<S: SerialDevice> {
    buffer: Packet,
    device: S,
}

impl<S: SerialDevice> Sender<S> {
    pub fn new(device: S) -> Self {
        Self {
            buffer: Packet::zeroed(),
            device,
        }
    }

    pub fn send(
        &mut self,
        data: &[u8],
        mut progress: impl FnMut(usize, usize),
    ) -> Result<(), Error<S::Error>> {
        let mut id = 1;
        match self.device.recv()? {
            CHECKSUM_REQUEST => {}
            ty => return Err(Error::BadPacketType(ty)),
        }

        for (i, chunk) in data.chunks(128).enumerate() {
            self.buffer.r#type = START_OF_HEADER;
            self.buffer.id = id;
            self.buffer.id_inverted = !id;
            self.buffer.data[..chunk.len()].copy_from_slice(chunk);
            self.buffer.crc = u16::to_be_bytes(checksum(&self.buffer.data));

            loop {
                for &byte in bytemuck::bytes_of(&self.buffer) {
                    self.device.send(byte)?;
                }

                match self.device.recv()? {
                    ACK => break,
                    NAK => continue,
                    CANCEL => return Err(Error::Canceled),
                    ty => return Err(Error::BadPacketType(ty)),
                }
            }

            id = id.wrapping_add(1);

            if i % 10 == 0 || i == data.len() / 128 {
                progress(i * 128, data.len());
            }
        }

        self.device.send(END_OF_TRANSMISSION)?;
        match self.device.recv()? {
            ACK => {}
            ty => return Err(Error::BadPacketType(ty)),
        }

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

impl SerialDevice for File {
    type Error = io::Error;

    fn recv(&mut self) -> Result<u8, Self::Error> {
        let mut byte = [0u8];
        while let 0 = self.read(&mut byte[..])? {}
        Ok(byte[0])
    }

    fn send(&mut self, c: u8) -> Result<(), Self::Error> {
        self.write_all(&[c])
    }
}

pub fn main() -> Result<(), Box<dyn error::Error>> {
    let args = clap::Command::new("xs")
        .arg(clap::arg!(device: <DEVICE>))
        .arg(clap::arg!(input: <INPUT>))
        .get_matches();

    let input_path = args.get_one::<String>("input").unwrap();
    let payload = fs::read(input_path).unwrap();

    let device_path = args.get_one::<String>("device").unwrap();

    let device = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NONBLOCK)
        .open(device_path)
        .unwrap();

    println!("Press enter to flash...");
    let _ = std::io::stdin().read(&mut [0u8])?;
    println!("Flashing...");

    let mut termios = termios::tcgetattr(&device).unwrap();

    termios.control_flags -= termios::ControlFlags::PARENB
        | termios::ControlFlags::CSTOPB
        | termios::ControlFlags::CSIZE
        | termios::ControlFlags::CRTSCTS;
    termios.control_flags |=
        termios::ControlFlags::CS8 | termios::ControlFlags::CREAD | termios::ControlFlags::CLOCAL;

    termios.local_flags -= termios::LocalFlags::ICANON
        | termios::LocalFlags::ECHO
        | termios::LocalFlags::ECHOE
        | termios::LocalFlags::ECHONL
        | termios::LocalFlags::ISIG;

    termios.input_flags -= termios::InputFlags::IGNBRK
        | termios::InputFlags::BRKINT
        | termios::InputFlags::PARMRK
        | termios::InputFlags::ISTRIP
        | termios::InputFlags::INLCR
        | termios::InputFlags::IGNCR
        | termios::InputFlags::ICRNL
        | termios::InputFlags::IXON
        | termios::InputFlags::IXOFF
        | termios::InputFlags::IXANY;

    termios.output_flags -= termios::OutputFlags::OPOST
        | termios::OutputFlags::ONLCR
        | termios::OutputFlags::OXTABS
        | termios::OutputFlags::ONOEOT
        | termios::OutputFlags::OCRNL
        | termios::OutputFlags::ONOCR;

    termios.control_chars[termios::SpecialCharacterIndices::VTIME as usize] = 0;
    termios.control_chars[termios::SpecialCharacterIndices::VMIN as usize] = 0;

    termios::cfsetspeed(&mut termios, termios::BaudRate::B115200)?;
    termios::tcsetattr(&device, termios::SetArg::TCSANOW, &termios)?;

    {
        let serial = device;
        let (mut read, mut write) = (serial.try_clone()?, serial);
        let (tx1, rx1) = std::sync::mpsc::channel::<()>();
        let (tx2, rx2) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::spawn(move || loop {
            let mut buf = [0u8; 64];

            match read.read(&mut buf[..]) {
                Ok(n) => {
                    if n == 1 && buf[0] == b'C' {
                        tx1.send(()).unwrap();
                        rx2.recv().unwrap();
                        continue;
                    }

                    for byte in &buf[..n] {
                        print!("{}", *byte as char);
                    }
                }
                Err(e) => println!("DBG: ERR: {:?}", e),
            }

            let _ = std::io::stdout().lock().flush();
        });

        std::thread::sleep(std::time::Duration::from_millis(1000));

        write.write_all(&[b'a'])?;

        std::thread::sleep(std::time::Duration::from_millis(2000));

        write.write_all(&[b'0', b'\r', b'\n'])?;

        rx1.recv().unwrap();

        let now = std::time::Instant::now();

        let mut sender = Sender::new(write);

        println!();
        sender
            .send(&payload[..], |current, total| {
                print!("\x1B[0K\r[");
                let percent = current as f32 / total as f32 * 10.0;
                for i in 1..11 {
                    if percent > i as f32 {
                        print!("=");
                    } else {
                        print!(" ");
                    }
                }

                const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
                let mut transferred = current as f32;
                let mut transferred_unit = 0;
                let mut total = total as f32;
                let mut total_unit = 0;

                while transferred / 1024.0 > 1.0 {
                    transferred /= 1024.0;
                    transferred_unit += 1;
                }

                while total / 1024.0 > 1.0 {
                    total /= 1024.0;
                    total_unit += 1;
                }

                print!(
                    "] {transferred:.02} {} / {total:.02} {}",
                    UNITS[transferred_unit], UNITS[total_unit]
                );
                let _ = std::io::stdout().flush();
            })
            .unwrap();

        println!("\nPayload sent, took: {:?}", now.elapsed());

        tx2.send(()).unwrap();
        let _ = thread.join();
    }

    //
    //     Sender::new(device)
    //         .send(&payload, |a, b| println!("{a}, {b}"))
    //         .unwrap();

    Ok(())
}
