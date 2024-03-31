#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod dtb;
mod panic;

use driver::uart;
use driver::watchdog;
use stdio::{gets, print, print_u32, println};

static mut INITRAMFS_ADDR: u32 = 0;
const MAX_COMMAND_LEN: usize = 0x400;

#[no_mangle]
pub extern "C" fn main() -> ! {
    uart::init();
    println("Hello, world!");
    print_mailbox_info();

    println("Dealing with dtb...");
    unsafe {
        INITRAMFS_ADDR = dtb::get_initrd_start().unwrap();
    }
    print("Initramfs address: ");
    print_u32(unsafe { INITRAMFS_ADDR });
    println("");

    let mut buf: [u8; MAX_COMMAND_LEN] = [0; MAX_COMMAND_LEN];
    loop {
        print("# ");
        gets(&mut buf);
        execute_command(&buf);
    }
}

fn execute_command(command: &[u8]) {
    if command.starts_with(b"\x00") {
        return;
    } else if command.starts_with(b"hello") {
        println("Hello, world!");
    } else if command.starts_with(b"help") {
        println("hello\t: print this help menu");
        println("help\t: print Hello World!");
        println("reboot\t: reboot the Raspberry Pi");
    } else if command.starts_with(b"reboot") {
        watchdog::reset(100);
    } else if command.starts_with(b"ls") {
        let rootfs = filesystem::cpio::CpioArchive::load(unsafe { INITRAMFS_ADDR } as *const u8);
        rootfs.print_file_list();
    } else if command.starts_with(b"cat") {
        let rootfs = filesystem::cpio::CpioArchive::load(unsafe { INITRAMFS_ADDR } as *const u8);
        let filename = &command[4..];
        if let Some(data) = rootfs.get_file(core::str::from_utf8(filename).unwrap()) {
            stdio::write(data);
        } else {
            print("File not found: ");
            stdio::puts(filename);
        }
    } else {
        print("Unknown command: ");
        stdio::puts(command);
    }
}

fn print_mailbox_info() {
    let revision = driver::mailbox::get_board_revision();
    let (lb, ub) = driver::mailbox::get_arm_memory();
    print("Board revision: ");
    print_u32(revision);
    println("");
    print("ARM memory: ");
    print_u32(lb);
    print(" - ");
    print_u32(ub);
    println("");
}
