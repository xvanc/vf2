use crate::time::Timeout;
use core::time::Duration;

pub struct Reg(usize);

impl Reg {
    pub const TX_HOLDING: Self = Self(0);
    pub const RX_BUFFER: Self = Self(0);
    pub const INTR_ENABLE: Self = Self(1);
    // pub const INTR_ID: Self = Self(2);
    pub const FIFO_CTRL: Self = Self(2);
    pub const LINE_CTRL: Self = Self(3);
    // pub const MODEM_CTRL: Self = Self(4);
    pub const LINE_STATUS: Self = Self(5);
    // pub const MODEM_STATUS: Self = Self(6);
    // pub const SCRATCH: Self = Self(7);

    pub const DIVISOR_LO: Self = Self(0);
    pub const DIVISOR_HI: Self = Self(1);
}

bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct InterruptEnable : u8 {
        const RX_DATA_AVAILABLE = 1 << 0;
        const TX_HOLDING_REGISTER_EMPTY = 1 << 1;
        const RX_LINE_STATUS = 1 << 2;
        const MODEM_STATUS = 1 << 3;
    }

    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct FifoControl : u8 {
        const ENABLE = 1 << 0;
        const RX_RESET = 1 << 1;
        const TX_RESET = 1 << 2;
        const DMA_MODE_SELECT = 1 << 3;
        const RX_TRIGGER_LO = 1 << 6;
        const RX_TRIGGER_HI = 1 << 7;
    }

    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct LineControl : u8 {
        const WORD_LENGTH_LO = 1 << 0;
        const WORD_LENGTH_HI = 1 << 1;
        const STOP_BITS = 1 << 2;
        const PARITY_ENABLE = 1 << 3;
        const EVEN_PARITY = 1 << 4;
        const STICK_PARITY = 1 << 5;
        const SET_BREAK = 1 << 6;
        const DIVISOR_LATCH_ACCESS = 1 << 7;
    }

    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct LineStatus : u8 {
        const DATA_READY = 1 << 0;
        const OVERRUN_ERROR = 1 << 1;
        const PARITY_ERROR = 1 << 2;
        const FRAMING_ERROR = 1 << 3;
        const BREAK = 1 << 4;
        const TX_HOLDING_REGISTER_EMPTY = 1 << 5;
        const TX_EMPTY = 1 << 6;

    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum UartError {
    TimedOut,
}

pub struct Uart {
    regs: *mut u32,
}

impl Uart {
    pub fn new() -> Uart {
        Self {
            regs: 0x10000000 as *mut u32,
        }
    }

    pub fn read_register(&self, reg: Reg) -> u8 {
        unsafe { self.regs.add(reg.0).read_volatile() as u8 }
    }

    pub fn write_register(&self, reg: Reg, val: u8) {
        unsafe { self.regs.add(reg.0).write_volatile(val as u32) };
    }

    pub fn set_interrupt_enable(&self, ier: InterruptEnable) {
        self.write_register(Reg::INTR_ENABLE, ier.bits());
    }

    pub fn set_fifo_control(&self, fcr: FifoControl) {
        self.write_register(Reg::FIFO_CTRL, fcr.bits());
    }

    pub fn line_control(&self) -> LineControl {
        LineControl::from_bits_retain(self.read_register(Reg::LINE_CTRL))
    }

    pub fn set_line_control(&self, lcr: LineControl) {
        self.write_register(Reg::LINE_CTRL, lcr.bits());
    }

    pub fn line_status(&self) -> LineStatus {
        LineStatus::from_bits_retain(self.read_register(Reg::LINE_STATUS))
    }

    pub fn set_baud(&self, baud: u32) -> Result<(), UartError> {
        let divisor = divisor_for_baud(baud);
        self.set_line_control(self.line_control() | LineControl::DIVISOR_LATCH_ACCESS);
        self.write_register(Reg::DIVISOR_LO, divisor[0]);
        self.write_register(Reg::DIVISOR_HI, divisor[1]);
        self.set_line_control(self.line_control() - LineControl::DIVISOR_LATCH_ACCESS);
        Ok(())
    }

    pub fn initialize(&self, baud: u32) -> Result<(), UartError> {
        self.set_interrupt_enable(InterruptEnable::empty());
        self.set_baud(baud)?;
        self.set_line_control(LineControl::WORD_LENGTH_HI | LineControl::WORD_LENGTH_LO);
        self.set_fifo_control(
            FifoControl::ENABLE
                | FifoControl::RX_RESET
                | FifoControl::TX_RESET
                | FifoControl::RX_TRIGGER_HI
                | FifoControl::RX_TRIGGER_LO,
        );

        Ok(())
    }

    pub fn flush_receiver(&self) -> Result<(), UartError> {
        while self.data_ready() {
            self.read_register(Reg::TX_HOLDING);
        }
        Ok(())
    }

    pub fn data_ready(&self) -> bool {
        self.line_status().contains(LineStatus::DATA_READY)
    }

    pub fn tx_holding_register_empty(&self) -> bool {
        self.line_status()
            .contains(LineStatus::TX_HOLDING_REGISTER_EMPTY)
    }

    pub fn receive_timeout(&self, timo: Duration) -> Result<u8, UartError> {
        let timeout = Timeout::start(timo);
        while !self.data_ready() {
            if timeout.expired() {
                return Err(UartError::TimedOut);
            }
        }
        Ok(self.read_register(Reg::RX_BUFFER))
    }

    pub fn receive(&self) -> Result<u8, UartError> {
        self.receive_timeout(Duration::MAX)
    }

    pub fn transmit(&self, byte: u8) -> Result<(), UartError> {
        while !self.tx_holding_register_empty() {
            core::hint::spin_loop();
        }
        self.write_register(Reg::TX_HOLDING, byte);
        Ok(())
    }
}

const CLOCK_FREQUENCY: u32 = 24000000;

fn divisor_for_baud(baud: u32) -> [u8; 2] {
    let divisor = u16::try_from(CLOCK_FREQUENCY / (baud * 16)).unwrap();
    divisor.to_le_bytes()
}
