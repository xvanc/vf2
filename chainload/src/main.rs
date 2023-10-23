#![no_std]
#![no_main]

mod time;
mod uart;
mod xmodem;

use core::{
    arch::{asm, global_asm},
    fmt::{self, Write},
};
use uart::Uart;

global_asm!(
    ".pushsection _dummy",
    include_str!(concat!(env!("OUT_DIR"), "/src/locore.s")),
    ".popsection",
    options(raw),
);

fn cease() -> ! {
    unsafe { asm!(".insn i 0x73, 0x0, x0, x0, 0x305", options(noreturn)) };
}

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes().iter().copied() {
            if byte == b'\n' {
                self.transmit(b'\r').map_err(|_| fmt::Error)?;
            }
            self.transmit(byte).map_err(|_| fmt::Error)?;
        }
        Ok(())
    }
}

macro_rules! uprintln {
    ($uart:expr $(,)?) => {
        $uart.write_str("\n").ok();
    };
    ($uart:expr, $($args:expr),* $(,)?) => {
        $uart.write_fmt(format_args!("{}\n", format_args!($($args),*))).ok();
    };
}

#[panic_handler]
fn rust_panic(info: &core::panic::PanicInfo) -> ! {
    let mut uart = Uart::new();
    uprintln!(uart, "panic: {info}");
    cease();
}

#[repr(C)]
struct TrapFrame {
    gpr: [usize; 32],
    mstatus: usize,
    mcause: usize,
    mepc: usize,
    mtval: usize,
    sp: usize,
}

#[no_mangle]
unsafe extern "C" fn trap_handler(tf: &mut TrapFrame) -> ! {
    let mut uart = Uart::new();

    uprintln!(
        uart,
        "\n\n\
        trap:\n\
        mcause: {:016x}\n\
        mepc: {:016x}\n\
        mtval: {:016x}\n\
        mstatus: {:016x}\n\
        sp: {:016x}\n",
        tf.mcause,
        tf.mepc,
        tf.mtval,
        tf.mstatus,
        tf.sp,
    );

    cease();
}

#[no_mangle]
unsafe extern "C" fn chainload_start() -> ! {
    let mut uart = Uart::new();
    uart.initialize(115200).unwrap();
    uprintln!(uart, "hello, world!");
    xmodem::receive(&mut uart, 0x40200000 as *mut u8).unwrap();
    cease();
}
