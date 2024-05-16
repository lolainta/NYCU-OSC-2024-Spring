use crate::exception::trap_frame::TRAP_FRAME;
use crate::thread::state;
use crate::thread::Thread;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::arch::asm;
use core::time::Duration;
use stdio::println;
pub struct Scheduler {
    pub current: Option<usize>,
    pub threads: Vec<Option<Box<Thread>>>,
    pub ready_queue: VecDeque<usize>,
    pub zombie_queue: VecDeque<usize>,
}

impl Scheduler {
    fn new() -> Self {
        Scheduler {
            current: None,
            threads: Vec::new(),
            ready_queue: VecDeque::new(),
            zombie_queue: VecDeque::new(),
        }
    }

    fn add(&mut self, thread: Box<Thread>) {
        self.threads.push(Some(thread));
        self.ready_queue.push_back(self.threads.len() - 1);
    }

    fn save_current(&mut self) {
        let current = self.current.unwrap();
        unsafe {
            self.threads[current].as_mut().unwrap().cpu_state = TRAP_FRAME.as_ref().unwrap().state;
        }
    }

    fn restore_next(&mut self, next: usize) {
        unsafe {
            TRAP_FRAME.as_mut().unwrap().state = self.threads[next].as_ref().unwrap().cpu_state;
        }
    }

    pub fn schedule(&mut self) {
        // println!("{} threads in ready queue", self.ready_queue.len());
        assert!(self.current.is_some());
        if self.ready_queue.is_empty() {
            // println!("No thread to schedule");
            return;
        }
        let current = self.current.unwrap();
        let range = self.threads[current].as_ref().unwrap().code_range;
        let pc = unsafe { TRAP_FRAME.as_ref().unwrap().state.pc };
        if range.0 <= pc && pc < range.1 {
            let next = self.ready_queue.pop_front().unwrap();
            if let Some(_) = self.threads[next].as_ref() {
                self.ready_queue.push_back(current);
                self.current = Some(next);
                println!("Switching from {} to {}", current, next);
                self.save_current();
                self.restore_next(next);
            } else {
                panic!("Thread {} is not available", next);
            }
        } else {
            panic!("Thread {} pc (0x{:x}) is not in range", current, pc);
        }
    }

    pub fn create_thread(&mut self, entry: extern "C" fn(), entry_size: usize, args: *mut u8) {
        let thread = Box::new(Thread::new(
            self.threads.len() as u32,
            0x2000,
            entry,
            entry_size,
            args,
        ));
        println!("Created thread {}", thread.id);
        self.add(thread);
        println!("Number of threads: {}", self.threads.len());
    }

    pub fn sched_timer(&mut self) {
        let tm = crate::timer::manager::get();
        tm.add_timer(
            Duration::from_secs_f64(1 as f64 / (1 << 5) as f64),
            Box::new(|| {
                get().schedule();
                get().sched_timer();
            }),
        );
    }

    pub fn run_threads(&mut self) {
        self.sched_timer();
        assert!(self.current.is_none());
        assert!(!self.ready_queue.is_empty());
        let next = self.ready_queue.pop_front().unwrap();
        self.current = Some(next);
        println!("Switching to {}", next);
        let entry = self.threads[next].as_ref().unwrap().entry;
        let stack_pointer = self.threads[next].as_ref().unwrap().stack as usize
            + self.threads[next].as_ref().unwrap().stack_size;
        unsafe {
            asm!(
                "msr spsr_el1, xzr",
                "msr elr_el1, {0}",
                "msr sp_el0, {1}",
                "eret",
                in(reg) entry,
                in(reg) stack_pointer,
            );
        }
    }
    pub fn fork(&mut self) -> u64 {
        self.save_current();
        let current = self.current.unwrap();
        let mut new_thread = self.threads[current].as_ref().unwrap().clone();
        new_thread.id = self.threads.len() as u32;
        new_thread.stack = unsafe {
            alloc::alloc::alloc(
                alloc::alloc::Layout::from_size_align(new_thread.stack_size, 16).unwrap(),
            ) as *mut u8
        };
        println!(
            "Thread {} stack: 0x{:x} ~ 0x{:x}",
            new_thread.id,
            new_thread.stack as u64,
            new_thread.stack as u64 + new_thread.stack_size as u64
        );
        new_thread.cpu_state.x[0] = 0;
        new_thread.cpu_state.sp = (self.threads[current].as_ref().unwrap().cpu_state.sp
            - self.threads[current].as_ref().unwrap().stack as u64
            + new_thread.stack as u64) as u64;
        self.add(new_thread);
        self.threads.last().unwrap().as_ref().unwrap().id as u64
    }

    pub fn exit(&mut self, status: u64) {
        let current = self.current.unwrap();
        self.threads[current].as_mut().unwrap().state = state::State::Zombie;
        self.zombie_queue.push_back(current);
        println!("Thread {} exited with status {}", current, status);
        self.current = None;
        self.sched_timer();
        if self.ready_queue.is_empty() {
            panic!("All threads exited");
        }
        if let Some(next) = self.ready_queue.pop_front() {
            self.current = Some(next);
            println!("Thread {} is scheduled", next);
            self.restore_next(next);
        }
    }
}

pub extern "C" fn get_current() -> *mut Thread {
    let current: *mut Thread;
    unsafe {
        asm!("mrs {0}, tpidr_el1", out(reg) current);
    }
    current
}

#[no_mangle]
pub extern "C" fn thread_wrapper() {
    let thread = get_current();
    let thread = unsafe { &mut *thread };
    (thread.entry)();
    println!("Thread {} exited", thread.id);
    panic!("Thread exited");
}

static mut SCHEDULER: Option<Scheduler> = None;

pub fn get() -> &'static mut Scheduler {
    unsafe { SCHEDULER.as_mut().unwrap() }
}

pub fn init() {
    unsafe {
        SCHEDULER = Some(Scheduler::new());
    }
}
