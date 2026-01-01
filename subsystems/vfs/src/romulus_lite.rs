use core::sync::atomic::{AtomicU8, Ordering};
use alloc::boxed::Box;

/// Romulus 容器：负责管理数据的两个版本
pub struct Romulus<T> {
    // 状态位：0 表示 views[0] 是真身，1 表示 views[1] 是真身
    state: AtomicU8,
    // 两个视图，第一天我们用 Box 模拟堆内存
    views: [Box<T>; 2],
}

impl<T: Clone> Romulus<T> {
    /// 初始化：将初始数据存入两个视图
    pub fn new(data: T) -> Self {
        Self {
            state: AtomicU8::new(0),
            views: [Box::new(data.clone()), Box::new(data)],
        }
    }

    /// 核心：无锁读取 (Wait-free Read)
    /// 无论写操作多疯狂，读操作永远只看当前的“真身”
    pub fn read(&self) -> &T {
        let idx = self.state.load(Ordering::Acquire) as usize;
        &self.views[idx]
    }

    /// 核心：原子事务写入 (Atomic Transaction)
    pub fn update<F>(&mut self, f: F) 
    where F: FnOnce(&mut T) {
        let current_idx = self.state.load(Ordering::Acquire) as usize;
        let back_idx = 1 - current_idx;

        // 1. 同步：将“真身”内容同步到“影子副本” (模拟持久化拷贝)
        *self.views[back_idx] = (*self.views[current_idx]).clone();

        // 2. 修改：在影子副本上执行你的文件系统逻辑 (比如添加文件)
        f(&mut self.views[back_idx]);

        // 3. 提交：瞬间翻转状态位！
        // 这一步之后，所有的 read() 都会立即看到新数据
        self.state.store(back_idx as u8, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;
    use std::thread;
    use std::sync::Arc;

    #[test]
    fn test_basic_read_update() {
        println!("test_basic_read_update: start");
        let mut r = Romulus::new(0);
        assert_eq!(*r.read(), 0);

        r.update(|val| {
            *val = 42;
        });

        assert_eq!(*r.read(), 42);
        println!("test_basic_read_update: !TEST FINISH!");
    }

    #[test]
    fn test_panic_rollback() {
        println!("test_panic_rollback: start");
        let mut r = Romulus::new(vec![]);
        
        // 1. Normal insert
        r.update(|v| v.push(1));
        assert_eq!(*r.read(), vec![1]);

        // 2. Simulate crash
        // Use a wrapper to allow catching unwind with mutable reference
        let mut wrapper = std::cell::UnsafeCell::new(r);
        
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            // Unsafe to get mutable reference, but valid for test logic simulation
            let r_ref = unsafe { &mut *wrapper.get() };
            r_ref.update(|v| {
                v.push(2);
                panic!("crash!");
            });
        }));

        assert!(result.is_err());
        
        // 3. Verify rollback
        let r_final = unsafe { &*wrapper.get() };
        // Should still be [1], not [1, 2]
        assert_eq!(*r_final.read(), vec![1]);
        println!("test_panic_rollback: !TEST FINISH!");
    }
}
