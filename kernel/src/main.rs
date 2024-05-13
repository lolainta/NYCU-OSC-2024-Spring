#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(btreemap_alloc)]
#![feature(duration_constructors)]

extern crate alloc;

mod allocator;
mod commands;
mod dtb;
mod exception;
mod kernel;
mod panic;
mod scheduler;
mod syscall;
mod thread;
mod timer;

use crate::thread::test_func;
use alloc::boxed::Box;
use allocator::buddy::BUDDY_SYSTEM;
use core::arch::asm;
use core::time::Duration;
use stdio::{debug, gets, print, println};

pub static mut INITRAMFS_ADDR: u32 = 0;

fn main() -> ! {
    boot();
    scheduler::init();
    println!("Kernel booted successfully!");
    // let scheduler = scheduler::get();
    // for i in 0..1 {
    // scheduler.create_thread(kernel_main, 0 as *mut u8);
    // scheduler.create_thread(test_func, 1 as *mut u8);
    // }

    // scheduler.schedule();

    // unsafe {
    //     let mut sp: u64;
    //     asm!("mov {}, sp", out(reg) sp);
    //     println!("Main thread sp: {:x}", sp);
    // }
    // allocator::utils::toggle_dynamic_verbose();
    commands::exec::exec(b"exec program.img");
    kernel_shell();
}

extern "C" fn kernel_main(_: *mut u8) {
    println!("Kernel main thread started!");
    kernel_shell();
}

fn kernel_shell() -> ! {
    const MAX_COMMAND_LEN: usize = 0x100;
    let mut buf: [u8; MAX_COMMAND_LEN] = [0; MAX_COMMAND_LEN];
    loop {
        print!("> ");
        gets(&mut buf);
        execute_command(&buf);
    }
}

fn boot() {
    println!("Hello, world!");
    print_mailbox_info();
    initramfs_init();
    buddy_init();
    timer::manager::init();
    print_boot_time();
}

fn execute_command(command: &[u8]) {
    if command.starts_with(b"\x00") {
        return;
    } else if command.starts_with(b"hello") {
        commands::hello::exec();
    } else if command.starts_with(b"help") {
        commands::help::exec();
    } else if command.starts_with(b"reboot") {
        commands::reboot::exec();
    } else if command.starts_with(b"ls") {
        commands::ls::exec();
    } else if command.starts_with(b"cat") {
        commands::cat::exec(&command);
    } else if command.starts_with(b"exec") {
        commands::exec::exec(&command);
    } else if command.starts_with(b"echo") {
        commands::echo::exec(&command);
    } else if command.starts_with(b"setTimeOut") {
        commands::set_time_out::exec(&command);
    } else if command.starts_with(b"buddy") {
        commands::buddy::exec();
    } else {
        println!(
            "Unknown command: {}",
            core::str::from_utf8(command).unwrap()
        );
    }
}

fn print_mailbox_info() {
    println!("Printing mailbox info...");
    let revision = driver::mailbox::get_board_revision();
    println!("Board revision: {:x}", revision);
    let (lb, ub) = driver::mailbox::get_arm_memory();
    println!("ARM memory: {:x} - {:x}", lb, ub);
}

fn initramfs_init() {
    unsafe {
        INITRAMFS_ADDR = dtb::get_initrd_start();
    }
    debug!("Initramfs address: {:#x}", unsafe { INITRAMFS_ADDR });
}

fn buddy_init() {
    unsafe {
        BUDDY_SYSTEM.init();
    }
    buddy_reserve_memory();
    allocator::utils::toggle_bump_verbose();
    unsafe {
        BUDDY_SYSTEM.print_info();
    }
}

fn buddy_reserve_memory() {
    let rsv_mem = dtb::get_reserved_memory();
    for (addr, size) in rsv_mem {
        unsafe {
            BUDDY_SYSTEM.reserve_by_addr_range(addr, addr + size);
        }
    }

    unsafe {
        BUDDY_SYSTEM.reserve_by_addr_range(0x0_0000, 0x8_0000); //
        BUDDY_SYSTEM.reserve_by_addr_range(0x6_0000, 0x8_0000); // kernel stack reserved
        BUDDY_SYSTEM.reserve_by_addr_range(0x8_0000, 0x10_0000); // kernel code reserved
    }

    unsafe {
        // initramfs reserved
        BUDDY_SYSTEM.reserve_by_addr_range(INITRAMFS_ADDR, INITRAMFS_ADDR + 0x1_0000);

        // bump allocator reserved
        BUDDY_SYSTEM.reserve_by_addr_range(
            allocator::config::BUMP_START_ADDR,
            allocator::config::BUMP_END_ADDR,
        );

        // rpi3 dtb reserved
        BUDDY_SYSTEM
            .reserve_by_addr_range(dtb::get_dtb_addr().0, dtb::get_dtb_addr().0 + 0x10_0000);
    }
}

fn print_boot_time() {
    let tm = crate::timer::manager::get();
    let now = tm.get_current();
    let freq = tm.get_frequency();
    println!("Frequency: {} Hz", freq);
    println!("Current time: {}", now);
    println!("Boot time: {} ms", now / (freq / 1000));
}
