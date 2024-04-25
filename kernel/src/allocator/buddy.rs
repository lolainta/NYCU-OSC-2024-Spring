use super::bump;
use alloc::collections::BTreeSet;
use core::alloc::{GlobalAlloc, Layout};
use stdio::println;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum BuddyState {
    Head(usize),
    Owned(usize),
    Allocated,
}

const MEMORY_START: u32 = 0x1000_0000;
const MEMORY_END: u32 = 0x1080_0000;
const FRAME_SIZE: usize = 0x1000;

const NFRAME: usize = (MEMORY_END - MEMORY_START) as usize / FRAME_SIZE;

#[derive(Clone, Copy)]
struct Frame {
    state: BuddyState,
}

const LAYER_COUNT: usize = 12;

pub struct BuddyAllocator {
    frames: [Frame; NFRAME],
    free_list: [BTreeSet<usize, bump::BumpAllocator>; LAYER_COUNT],
}

impl BuddyAllocator {
    pub const fn new() -> Self {
        const EMPTY: BTreeSet<usize, bump::BumpAllocator> = BTreeSet::new_in(bump::BumpAllocator);
        Self {
            frames: [Frame {
                state: BuddyState::Allocated,
            }; NFRAME],
            free_list: [EMPTY; LAYER_COUNT],
        }
    }
    pub unsafe fn init(&mut self) {
        println!("Initializing buddy allocator");
        println!("Frame count: {}", NFRAME);
        assert!(
            MEMORY_START % (1 << LAYER_COUNT) as u32 == 0,
            "Memory start 0x{:x} must be aligned to frame size 0x{:x}",
            MEMORY_START,
            FRAME_SIZE
        );
        for idx in 0..NFRAME {
            if let BuddyState::Owned(_) = BUDDY_SYSTEM.frames[idx].state {
                continue;
            }
            for layer in (0..LAYER_COUNT).rev() {
                if idx % (1 << layer) == 0 && (idx + (1 << layer) <= NFRAME) {
                    BUDDY_SYSTEM.frames[idx].state = BuddyState::Head(layer);
                    BUDDY_SYSTEM.free_list[layer].insert(idx);
                    for i in 1..(1 << layer) {
                        BUDDY_SYSTEM.frames[idx + i].state = BuddyState::Owned(i);
                    }
                    println!("Initialized frame {} at layer {}", idx, layer);
                    println!("Frame state: {:?}", BUDDY_SYSTEM.frames[idx].state);
                    break;
                }
            }
        }
        println!("Free list: {:?}", BUDDY_SYSTEM.free_list);
    }

    fn faddr(&self, idx: usize) -> u32 {
        MEMORY_START + (idx * FRAME_SIZE) as u32
    }

    pub unsafe fn alloc_frame(&mut self, idx: usize) {
        assert!(idx < NFRAME);
        let layer = match BUDDY_SYSTEM.frames[idx].state {
            BuddyState::Head(l) => l,
            _ => panic!("Invalid state, expected Head"),
        };
        assert!(BUDDY_SYSTEM.free_list[layer].contains(&idx));
        for i in 0..(1 << layer) {
            BUDDY_SYSTEM.frames[idx + i].state = BuddyState::Allocated;
        }
        BUDDY_SYSTEM.free_list[layer].remove(&idx);
    }

    pub unsafe fn get_by_layout(&mut self, size: usize, align: usize) -> Option<usize> {
        let mut layer = 0;
        if align < FRAME_SIZE {
            layer = 0;
        } else {
            while (1 << layer) < align {
                layer += 1;
            }
        }
        while (1 << layer) * FRAME_SIZE < size {
            layer += 1;
        }
        if layer < LAYER_COUNT {
            BUDDY_SYSTEM.alloc_by_layer(layer)
        } else {
            None
        }
    }

    unsafe fn split_frame(&mut self, idx: usize) {
        let layer = match BUDDY_SYSTEM.frames[idx].state {
            BuddyState::Head(l) => l,
            _ => panic!("Invalid state, expected Head"),
        };
        assert!(BUDDY_SYSTEM.free_list[layer].contains(&idx));
        let cur = idx;
        BUDDY_SYSTEM.free_list[layer].remove(&cur);
        let buddy = idx ^ (1 << layer - 1);
        BUDDY_SYSTEM.frames[idx].state = BuddyState::Head(layer - 1);
        for i in 1..(1 << layer - 1) {
            BUDDY_SYSTEM.frames[idx + i].state = BuddyState::Owned(idx);
        }
        BUDDY_SYSTEM.frames[buddy].state = BuddyState::Head(layer - 1);
        for i in 1..(1 << layer - 1) {
            BUDDY_SYSTEM.frames[buddy + i].state = BuddyState::Owned(buddy);
        }
        BUDDY_SYSTEM.free_list[layer - 1].insert(idx);
        BUDDY_SYSTEM.free_list[layer - 1].insert(buddy);
        println!("Split frame {} into {} and {}", idx, idx, buddy);
    }

    unsafe fn get_by_layer(&mut self, layer: usize) -> Option<usize> {
        match BUDDY_SYSTEM.free_list[layer].first() {
            Some(idx) => Some(*idx),
            None => {
                if let Some(idx) = BUDDY_SYSTEM.get_by_layer(layer + 1) {
                    BUDDY_SYSTEM.split_frame(idx);
                    BUDDY_SYSTEM.get_by_layer(layer)
                } else {
                    None
                }
            }
        }
    }

    unsafe fn alloc_by_layer(&mut self, layer: usize) -> Option<usize> {
        if let Some(idx) = BUDDY_SYSTEM.get_by_layer(layer) {
            BUDDY_SYSTEM.alloc_frame(idx);
            Some(idx)
        } else {
            None
        }
    }

    unsafe fn free_by_layout(&mut self, ptr: *mut u8, size: usize, align: usize) {
        let addr = ptr as u32;
        let idx = (addr - MEMORY_START) as usize / FRAME_SIZE;
        let mut layer = 0;
        if align < FRAME_SIZE {
            layer = 0;
        } else {
            while (1 << layer) < align {
                layer += 1;
            }
        }
        while (1 << layer) * FRAME_SIZE < size {
            layer += 1;
        }
        println!("Free frame {} at layer {}", idx, layer);
        BUDDY_SYSTEM.free_by_idx(idx, layer);
    }

    unsafe fn free_by_idx(&mut self, mut idx: usize, mut layer: usize) {
        loop {
            let buddy = idx ^ (1 << layer);
            if buddy >= NFRAME {
                println!("Buddy out of range");
                break;
            }
            match BUDDY_SYSTEM.frames[buddy].state {
                BuddyState::Head(l) => {
                    if l == layer {
                        BUDDY_SYSTEM.free_list[layer].remove(&buddy);
                        println!("Merged frame {} and {}", idx, buddy);
                        layer += 1;
                        idx = idx & !((1 << layer) - 1);
                        continue;
                    } else {
                        break;
                    }
                }
                BuddyState::Owned(ord) => {
                    assert!(ord == 0);
                    layer += 1;
                }
                BuddyState::Allocated => {
                    break;
                }
            }
        }
        BUDDY_SYSTEM.frames[idx].state = BuddyState::Head(layer);
        for i in 1..(1 << layer) {
            BUDDY_SYSTEM.frames[idx + i].state = BuddyState::Owned(idx);
        }
        BUDDY_SYSTEM.free_list[layer].insert(idx);
        // println!("New free idx: {}", idx);
    }
}

unsafe impl GlobalAlloc for BuddyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        let ret = BUDDY_SYSTEM.get_by_layout(size, align);
        if let Some(idx) = ret {
            let addr = BUDDY_SYSTEM.faddr(idx);
            println!("Allocated frame {} {:?} at 0x{:x}", idx, layout, addr);
            assert!(addr % align as u32 == 0);
            println!("Free list: {:?}", BUDDY_SYSTEM.free_list);
            addr as *mut u8
        } else {
            panic!("Out of memory");
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        let align = layout.align();
        BUDDY_SYSTEM.free_by_layout(ptr, size, align);
    }
}

pub static mut BUDDY_SYSTEM: BuddyAllocator = BuddyAllocator::new();
