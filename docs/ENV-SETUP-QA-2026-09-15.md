# Alien 编译环境部署问答日志

**日期**：2026-09-15
**环境**：Parallels Desktop 虚拟机，Ubuntu 22.04（主机名 `ubuntu-linux-22-04-02-desktop`），仓库 `~/Alien`

---

## 这份日志是什么

在 Ubuntu 22.04 虚拟机里把 Alien 从「clone 下来」推进到「能编译内核 + 用户态程序」的过程记录。

按 **现象 → 排查 → 处理 → 实际结果** 四段式记，一条一个问题。

**写作原则**：只写实际执行过、实际看到输出的步骤。凡是没验证过的推断、没查清的机制，都单独标注出来，不写成结论。

**要提前说明的**：这个环境搭建**只是让代码能编译**，跟 DBFS2 文件系统本身的功能、性能、正确性**没有任何关系**。编译通过不代表文件系统能跑、能挂载、能正确读写。这一点在下面每一条的"结果"里都没有夸大。

---

## Q1. `wget` 下载 QEMU 报 DNS 解析失败，但能 ping 通 8.8.8.8

**现象**

```bash
wget https://download.qemu.org/qemu-7.0.0.tar.xz
# Resolving download.qemu.org... failed: Temporary failure in name resolution.
```

但：

```bash
ping -c 3 8.8.8.8
# 正常
```

**排查**

网络没断，是 DNS 解析的问题。看当时的解析配置：

```bash
resolvectl status
# Current DNS Server: fe80::21c:42ff:fe00:18
# DNS Servers: 10.211.55.1 fe80::21c:42ff:fe00:18
```

**处理**

手动给 Parallels 网卡 `enp0s5` 指定公共 DNS：

```bash
sudo resolvectl dns enp0s5 8.8.8.8 1.1.1.1
```

**实际结果**

```bash
ping -c 3 download.qemu.org
# download.qemu.org -> 89.187.187.x   ← 解析成功
```

DNS 问题解决。

> ⚠️ 这是临时设置，重启或网卡重连后可能失效，需要重新执行。

---

## Q2. DNS 修好后 QEMU 的 HTTPS 下载仍然失败

**现象**

```bash
wget https://download.qemu.org/qemu-7.0.0.tar.xz
# Connecting to download.qemu.org|...|:443... connected.
# Unable to establish SSL connection.
```

`curl` 同样：

```text
OpenSSL SSL_connect: Connection reset by peer
```

**排查**

TCP 443 能建连（`connected`），但 TLS 握手被重置。已经不是 DNS 问题。

**处理**

**没有继续排查 SSL/TLS 层面。**

当时的实际情况是：环境里已经有了

```text
~/qemu
~/qemu-7.0.0
~/qemu-7.0.0.tar.xz
```

**实际结果**

QEMU 源码可用，绕过了下载这一步。

> ⚠️ **这一条没有真正解决**。TLS 握手被重置的原因没查（可能是中间网络、可能是站点限制）。
> 源码是怎么来的这段记录里没有写，不是通过这条 `wget` 下载成功的。

---

## Q3. 编译报 `can't find crate for core`

**现象**

```text
error[E0463]: can't find crate for `core`
the `riscv64gc-unknown-none-elf` target may not be installed
```

**排查**

```bash
rustup show
# active toolchain: nightly-2025-05-20-x86_64-unknown-linux-gnu
# installed targets: x86_64-unknown-linux-gnu    ← 只有宿主 target
```

RISC-V 裸机 target 没装。

**处理**

```bash
rustup target add riscv64gc-unknown-none-elf --toolchain nightly-2025-05-20
```

**实际结果**

Alien kernel 的 Rust 部分编译通过。

---

## Q4. 编译 C apps 报 `riscv64-linux-musl-gcc: not found`

**现象**

```text
Building C apps
riscv64-linux-musl-gcc -static ...
/bin/sh: 1: riscv64-linux-musl-gcc: not found
```

**排查**

`~/Downloads` 里已经有工具链压缩包：

```text
riscv64-linux-musl-cross.tgz
riscv64-linux-musl-native.tgz
```

用的是 `cross` 那份，解压到 `~/Downloads/riscv64-linux-musl-cross`。

**先验证工具链本身是好的**（这一步很关键，避免误判成工具链损坏）：

```bash
~/Downloads/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc -static test.c -o test
file test
# ELF 64-bit LSB pie executable, UCB RISC-V, RVC, double-float ABI,
# version 1 (SYSV), static-pie linked, not stripped
```

→ 交叉编译器本身完全正常。

**处理**

设 PATH：

```bash
export RISCV=/home/parallels/Downloads/riscv64-linux-musl-cross
export PATH=$RISCV/bin:$PATH
which riscv64-linux-musl-gcc
# /home/parallels/Downloads/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc
```

另外 Alien 的构建脚本调用的是 `riscv64-linux-musl-cc`，而工具链提供的是 `riscv64-linux-musl-gcc`，补一个软链：

```bash
cd ~/Downloads/riscv64-linux-musl-cross/bin
ln -s riscv64-linux-musl-gcc riscv64-linux-musl-cc
```

**实际结果**

编译器能找到了。

> ⚠️ **`export` 只在当前终端有效**，这是 Q9 那个问题的根源。

---

## Q5. 链接报 `cannot find crt1.o / crti.o / crtbegin.o / -lunwind / crtend.o / crtn.o`

**现象**

```text
cannot find crt1.o
cannot find crti.o
cannot find crtbegin.o
cannot find -lunwind
cannot find crtend.o
cannot find crtn.o
```

**排查**

这个报错很容易误判成「musl 工具链坏了」。实际去查了 sysroot：

```bash
riscv64-linux-musl-cc -print-sysroot
# .../riscv64-linux-musl-cross/bin/../riscv64-linux-musl    ← GCC 知道自己的 sysroot

riscv64-linux-musl-cc -print-file-name=crt1.o
# .../riscv64-linux-musl/lib/crt1.o                          ← 找得到
riscv64-linux-musl-cc -print-file-name=crti.o
# .../lib/gcc/riscv64-linux-musl/11.2.1/crti.o               ← 找得到
riscv64-linux-musl-cc -print-file-name=crtbegin.o
#                                                            ← 也找得到
```

**结论：musl 工具链不缺文件。**

**处理**

原来的 `.cargo/config.toml` 只有：

```toml
cargo-features = ["edition2024"]

[profile.release]
strip = true
debug = false
```

给 musl target 显式指定 linker：

```toml
[target.riscv64gc-unknown-linux-musl]
linker = "/home/parallels/Downloads/riscv64-linux-musl-cross/bin/riscv64-linux-musl-cc"
```

**实际结果**

不再报这几个找不到，构建推进到了下一阶段。

> ⚠️ **机制没有完全查清**。能确定的是：不是工具链缺文件（已用 `-print-file-name` 逐一验证）；
> 但「为什么 Rust/Cargo 原先找不到」这一点没有定位到具体原因，只是指定 linker 后不再复现。
> 这条记录里不下"根因是什么"的结论。

---

## Q6. Rust 编译报 `invalid register 'x10': unknown register`

**现象**

```text
error: invalid register `x10`: unknown register
```

涉及 `x10 x11 x12 x13 x14 x15 x17`，位置 `user/userlib/src/syscall.rs`。

**排查**

当前 Rust nightly 对 RISC-V inline asm 的寄存器名要求变了：不接受数字名 `xN`，要用 ABI 名。

**处理**

逐个替换：

| 原名 | 改为 |
|---|---|
| x10 | a0 |
| x11 | a1 |
| x12 | a2 |
| x13 | a3 |
| x14 | a4 |
| x15 | a5 |
| x17 | a7 |

即：

```rust
inlateout("a0") args[0] => ret,
in("a1") args[1],
in("a2") args[2],
in("a3") args[3],
in("a4") args[4],
in("a5") args[5],
in("a7") id
```

**实际结果**

编译通过。

---

## Q7. `CStr::from_ptr()` 报类型不匹配

**现象**

```text
expected `*const i8`, found `*const u8`
```

位置 `user/userlib/src/fs/mod.rs:100`。

**排查**

原代码 `core::ffi::CStr::from_ptr(name)`，而 `name` 是 `*const u8`；当前 Rust 的 `CStr::from_ptr` 要 `*const i8`。

**处理**

```rust
let name = core::ffi::CStr::from_ptr(name as *const i8);
```

**实际结果**

编译通过。

---

## Q8. Rust 用户态程序最终编译成功了吗

**实际结果：成功。**

输出：

```text
Finished `release` profile [optimized] target(s) in 0.28s
Moving apps to ../diskfs/bin
```

之前也出现过：

```text
Compiling final_test
Finished release
```

说明这条链路是通的：

```text
Alien kernel → Rust userlib → Rust user apps
```

---

## Q9. C apps 又报 `riscv64-linux-musl-gcc: not found`（和 Q4 不是同一个问题）

**现象**

```text
Building C apps
riscv64-linux-musl-gcc -static -o eventfd_test eventfd_test.c
/bin/sh: 1: riscv64-linux-musl-gcc: not found
```

**排查**

和 Q5 的 linker 错误**不是一回事**。

原因：Q4 里是在**当前终端手动** `export PATH=...`，所以那个 shell 能找到 gcc；但重新开终端、或者 `make` 起了新的子 shell 时，环境变量没有持久化，PATH 里就没有工具链的 `bin` 目录。

**处理**

在当前终端重新执行：

```bash
export RISCV=/home/parallels/Downloads/riscv64-linux-musl-cross
export PATH=$RISCV/bin:$PATH
which riscv64-linux-musl-gcc
```

然后：

```bash
cd ~/Alien
make
```

**实际结果**

**到记录结束时，这一条还没有走通。** 每次新开终端都要重新 `export`，没有做成持久化（比如写进 `~/.bashrc` 或在 Makefile 里写绝对路径）。

---

## 最终状态

```text
Ubuntu 22.04 (Parallels VM)
├── DNS 解析                    ✅ 已解决（临时设置，重启可能失效）
├── QEMU 源码就位               ⚠️ 可用，但 HTTPS 下载通路没通，来源未记录
├── Rust nightly                ✅
├── riscv64gc bare-metal target ✅
├── musl 交叉工具链             ✅（已实测可编出 RISC-V 可执行文件）
├── Cargo linker 配置           ⚠️ 配了 linker 后通过，根因未定位
├── Alien kernel 编译           ✅
├── Rust userlib                ✅
├── Rust user apps              ✅
└── C apps 编译                 ⏳ 卡在 PATH 未持久化
```

---

## 明确没做完 / 没查清的部分

如实列出，避免后面误以为都好了：

1. **C apps 编译未跑通**。卡在 PATH 未持久化（Q9）。要收尾的话：把 `export` 写进 `~/.bashrc`，或者直接在 C apps 的 Makefile 里用工具链绝对路径。
2. **QEMU 的 HTTPS 下载没修**（Q2）。只是环境里恰好已有源码，绕过去了。换台机器会再撞上。
3. **`crt1.o` 那个报错的根因没定位**（Q5）。只验证了"不是工具链缺文件"，配了 linker 后不复现，但为什么之前找不到没查清。
4. **DNS 是临时设置**（Q1）。`resolvectl` 的设置重启后可能丢。
5. **到这里为止只做到了"能编译"**。没有跑过 QEMU 启动，没有挂载过任何文件系统，没有跑过任何功能测试或性能测试。**编译通过 ≠ 系统能跑。**

---

## 附：环境就绪后的一组命令

```bash
export RISCV=/home/parallels/Downloads/riscv64-linux-musl-cross
export PATH=$RISCV/bin:$PATH
which riscv64-linux-musl-gcc      # 应指向 RISCV 下的路径

cd ~/Alien
make
```
