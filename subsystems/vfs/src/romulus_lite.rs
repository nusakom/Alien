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

    #[test]
    fn test_basic_read_update() {
        let mut r = Romulus::new(0);
        assert_eq!(*r.read(), 0);

        r.update(|val| {
            *val = 42;
        });

        assert_eq!(*r.read(), 42);
    }

    #[test]
    fn test_panic_rollback() {
        let mut r = Romulus::new(vec![]);
        
        // 正常插入
        r.update(|v| v.push(1));
        assert_eq!(*r.read(), vec![1]);

        // 模拟崩溃
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            // 这里我们需要用 UnsafeCell 或者 RefCell 来绕过 &mut 借用检查吗？
            // 不，因为 update 拿的是 &mut self，在闭包里 panic 会导致 update 函数未完成，state 未翻转。
            // 但是我们要在一个已经在 update 里的结构体上再次调用 read 需要 careful ownership。
            // 实际上测试里我们持有 owner。
            // 可是 catch_unwind 里的闭包如果是 FnOnce, 需要 move r 进去？
            // 我们可以用 Arc<Mutex<Romulus>> 或者 直接 RefCell? 
            // 这里的 Romulus::update 需要 &mut self.
            // 我们可以仅测试 panic 发生后，外部 catch 到 panic，然后再次查看 r。
            // 但 panic unwind 可能会毒化 r 吗？ Rust 的 panic unwinding 默认是安全的。
            // 只是我们需要 owner 还在外面。
            
            // 下面这个闭包借用了 r (mutably)，如果 panic，borrow 结束了吗？
            // 这是一个 borrow checker 的问题。
            // 我们可以把 update 包装在一个 helper 函数里，helper 接受 &mut Romulus。
            
            // 这是一个 Rust 测试技巧问题。
            // 让我们尝试最直接的方法。
            
            let mut wrapper = &mut r;
            match panic::catch_unwind(panic::AssertUnwindSafe(move || {
                 wrapper.update(|v| {
                    v.push(2);
                    panic!("oops");
                });
            })) {
                 Ok(_) => {},
                 Err(_) => {},
            }
        }));
        
        // 上面的写法有点绕，因为 panic::catch_unwind 要求闭包是 UnwindSafe 且 'static (如果 capture 引用)。
        // 捕获 &mut T 是不满足 'static 的。
        // 所以我们可能需要把 Romulus 放到一个可以 clone 的结构如 Arc<Mutex> 里或者直接 move 进去（但那样就拿不出来了）。
        // 不过，我们可以测试逻辑上的等价物：手动模拟 update 的一半过程。
        // 但我们要测的是 Romulus::update 的抗 panic 能力。
        
        // 更好的方法：
        // 只是测试 "如果 closure panic，state 没变"。
        // 我们可以只测试 update 函数逻辑:
        // 在 update 内部, *self.views[back_idx] 被改了, state 还没改.
        // 如果 panic, catch_unwind 捕获.
        // 问题是 Rust 在 panic 时会 drop 栈变量。Romulus 本身在外面，没事。
        // 但借用检查器会认为 panic 发生时借用还持有？
        // 其实 panic unwind 后借用就释放了。
        
        // 让我们试试这个简单的测试逻辑：
        // 由于测试环境可以用 std，我们用 std::panic::catch_unwind
    }
    
    #[test]
    fn test_panic_simulation() {
         // 我们不能直接对借用的 &mut r 做 catch_unwind，因为生命周期原因。
         // 我们换个思路：如果 update 内部 panic，外部能拿到 r 吗？
         // 只要 catch_unwind 包裹住 update 调用即可。
         // 但 update(&mut self) 也就意味着 catch_unwind 闭包必须 capture &mut self。
         // 这要求 &mut self 是 'static 的（如果用 catch_unwind thread-local 限制较少，但这里是普通）。
         // 或者是 AssertUnwindSafe.
         
         let mut r = Romulus::new(vec![1, 2, 3]);
         
         // 我们需要用一个小技巧来绕过 'static 限制，或者直接用 AssertUnwindSafe。
         // 但 &mut T 还是有生命周期问题。
         
         // 解决方案：使用 AssertUnwindSafe 和 scoped thread 或者直接只要编译器允许。
         // 简单粗暴点：
         let mut r = Box::new(Romulus::new(vec![1]));
         let r_ptr = &mut *r as *mut Romulus<Vec<i32>>;
         
         let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
             unsafe {
                 (*r_ptr).update(|v| {
                     v.push(2);
                     panic!("crash!");
                 });
             }
         }));

         assert!(result.is_err());
         
         // 验证：数据应该是 [1]，而不是 [1, 2]
         assert_eq!(*r.read(), vec![1]);
    }
}
