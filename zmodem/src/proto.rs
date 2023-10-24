use bytemuck::{Pod, Zeroable};

pub mod consts {
    pub const SOH: u8 = 0x01;
    pub const STX: u8 = 0x02;
    pub const EOT: u8 = 0x04;
    pub const ENQ: u8 = 0x05;
    pub const ACK: u8 = 0x06;
    pub const LF: u8 = 0x0a;
    pub const CR: u8 = 0x0d;
    pub const DLE: u8 = 0x10;
    pub const XON: u8 = 0x11;
    pub const XOFF: u8 = 0x13;
    pub const NAK: u8 = 0x15;
    pub const CAN: u8 = 0x18;

    pub const ZPAD: u8 = 0x2a;
    pub const ZDLE: u8 = CAN;
    pub const ZDLEE: u8 = 0x58;

    pub const ZCRCE: u8 = 0x68;
    pub const ZCRCG: u8 = 0x69;
    pub const ZCRCQ: u8 = 0x6a;
    pub const ZCRCW: u8 = 0x6b;
    pub const ZRUB0: u8 = 0x6c; // 7f
    pub const ZRUB1: u8 = 0x6d; // ff

    pub const ZVBIN: u8 = 0x61;
    pub const ZVHEX: u8 = 0x62;
    pub const ZVBIN32: u8 = 0x63;
    pub const ZVBINR32: u8 = 0x64;

    pub const ZRESC: u8 = 0x7e;
}

macro_rules! enum_struct {
    ($(
        $(#[$meta:meta])*
        $vis:vis struct $name:ident : $type:ty {$(
            $(#[$var_meta:meta])*
            const $var_name:ident = $var_value:expr;
        )*}
    )*) => {$(
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Pod, Zeroable, Eq, PartialEq)]
        $vis struct $name(pub $type);

        impl $name {$(
            $(#[$var_meta])*
            pub const $var_name: Self = Self($var_value);
        )*}

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                if let Some(name) = match *self {
                    $(
                        Self::$var_name => Some(stringify!($var_name)),
                    )*
                    _ => Option::<&str>::None,

                } {
                    f.write_str(name)?;
                } else {
                    write!(f, concat!(stringify!($name), "("))?;
                    self.0.fmt(f)?;
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    )*};
}

enum_struct! {
    pub struct FrameEncoding : u8 {
        const BIN16  = b'A';
        const HEX    = b'B';
        const BIN32  = b'C';
        const BINR32 = b'D';
    }

    pub struct FrameType : u8 {
        const ZRQINIT    = 0x00;
        const ZRINIT     = 0x01;
        const ZSINIT     = 0x02;
        const ZACK       = 0x03;
        const ZFILE      = 0x04;
        const ZSKIP      = 0x05;
        const ZNAK       = 0x06;
        const ZABORT     = 0x07;
        const ZFIN       = 0x08;
        const ZRPOS      = 0x09;
        const ZDATA      = 0x0a;
        const ZEOF       = 0x0b;
        const ZFERROR    = 0x0c;
        const ZCRC       = 0x0d;
        const ZCHALLENGE = 0x0e;
        const ZCOMPL     = 0x0f;
        const ZCAN       = 0x10;
        const ZFREECNT   = 0x11;
        const ZCOMMAND   = 0x12;
        const ZSTDERR    = 0x13;
    }

    pub struct PacketType : u8 {
        const ZCRCE = 0x68;
        const ZCRCG = 0x69;
        const ZCRCQ = 0x6a;
        const ZCRCW = 0x6b;
        const ZRUB0 = 0x6c;
        const ZRUB1 = 0x6d;
    }
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Pod, Zeroable)]
    pub struct ReceiverCapabilities : u16 {
        const CANFDX  = 0x01;
        const CANOVIO = 0x02;
        const CANBRK  = 0x04;
        const CANCRY  = 0x08;
        const CANLZW  = 0x10;
        const CANFC32 = 0x20;
        const ESCCTL  = 0x40;
        const ESC8    = 0x80;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FrameHeader {
    pub encoding: FrameEncoding,
    pub r#type: FrameType,
    pub data: [u8; 4],
}

impl FrameHeader {
    pub fn new(encoding: FrameEncoding, r#type: FrameType) -> FrameHeader {
        Self {
            encoding,
            r#type,
            data: [0, 0, 0, 0],
        }
    }

    pub fn set_flags(mut self, flags: u32) -> Self {
        self.data = flags.to_be_bytes();
        self
    }

    pub fn set_count(mut self, count: u32) -> Self {
        self.data = count.to_le_bytes();
        self
    }
}
