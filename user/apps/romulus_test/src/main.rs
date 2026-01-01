#![no_main]
#![no_std]

use Mstd::{
    println,
    process::exit,
    system_shutdown,
};

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicU8, Ordering};

/// Romulus Copy-Paste from kernel for user-space testing
/// Romulus Container: Manages two versions of data for atomic updates
pub struct Romulus<T> {
    // 0 means views[0] is active, 1 means views[1] is active
    state: AtomicU8,
    views: [Box<T>; 2],
}

impl<T: Clone> Romulus<T> {
    pub fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(0),
            views: [Box::new(data.clone()), Box::new(data)],
        }
    }

    pub fn read(&self) -> &T {
        let idx = self.state.load(Ordering::Acquire) as usize;
        &self.views[idx]
    }

    pub fn update<F>(&mut self, f: F) 
    where F: FnOnce(&mut T) {
        let current_idx = self.state.load(Ordering::Acquire) as usize;
        let back_idx = 1 - current_idx;

        // 1. Sync
        *self.views[back_idx] = (*self.views[current_idx]).clone();

        // 2. Modify
        f(&mut self.views[back_idx]);

        // 3. Commit
        self.state.store(back_idx as u8, Ordering::Release);
    }

    /// Helper for whitebox testing of atomicity
    /// Simulates a crash after modification but before commit
    pub fn update_simulate_crash<F>(&mut self, f: F) 
    where F: FnOnce(&mut T) {
        let current_idx = self.state.load(Ordering::Acquire) as usize;
        let back_idx = 1 - current_idx;

        // 1. Sync
        *self.views[back_idx] = (*self.views[current_idx]).clone();

        // 2. Modify
        f(&mut self.views[back_idx]);

        // 3. CRASH! (We just return without flipping state)
        println!("Simulating Crash! State not flipped.");
    }
}

#[no_mangle]
fn main() -> isize {
    println!("Starting Romulus Test Suite (User Space)");

    test_basic_read_update();
    test_simulated_crash_atomicity();

    println!("!TEST FINISH!");
    // system_shutdown(); // Optional, but typical for test apps
    0
}

fn test_basic_read_update() {
    println!("Test: Basic Read/Update");
    let mut r = Romulus::new(10);
    if *r.read() != 10 {
        println!("FAILED: Initial value mismatch");
        exit(1);
    }

    r.update(|v| *v = 20);

    if *r.read() != 20 {
        println!("FAILED: Update not reflected");
        exit(1);
    }
    println!("PASSED: Basic Read/Update");
}

fn test_simulated_crash_atomicity() {
    println!("Test: Atomicity (Simulated Crash)");
    let mut r = Romulus::new(vec![1, 2, 3]);

    // We try to push 4, but "crash" before commit
    r.update_simulate_crash(|v| {
        v.push(4);
    });

    // Validating rollback: The reader should still see [1, 2, 3]
    let current = r.read();
    if current.len() != 3 || current[2] != 3 {
        println!("FAILED: Initial state corrupted! Found: {:?}", current);
        exit(1);
    }
    
    // Check if 4 is present (should NOT be)
    if current.contains(&4) {
         println!("FAILED: Uncommitted data leaked! Found: {:?}", current);
         exit(1);
    }

    println!("PASSED: Atomicity verified. Reader sees consistent state despite 'crash'.");
}
