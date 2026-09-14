# Alien

A simple operating system implemented in rust. The purpose is to explore how to use modules to build a complete os, so the system is composed of a series of independent modules. At present, the system already supports user-mode programs and some simple functions.

<img src="assert/image-20230815132104606.png" alt="image-20230815132104606" style="zoom:50%;" />

## Project Structure

```
├── LICENSE
├── Makefile                (编译命令)
├── README.md               (readme)
├── apps                    (rust程序)
├── assert
├── kernel                  (核心子系统)
├── doc                     (开发文档与内核相关模块文档)
├── subsystems							
    ├── arch            (riscv相关代码)
    ├── platform        (平台相关代码)
    ├── config		    (内核配置)
    ├── devices         (设备注册管理)
    ├── drivers         (设备驱动合集)
    ├── unwinder        (内核panic处理)
    ├── vfs             (虚拟文件系统)
    ├── interrupt       (外中断注册管理)
    ├── ipc             (进程间通信模块)
    ├── mem          	(内存管理)
    ├── knet            (网络模块)
    ├── ksync           (内核锁实现)
    ├── timer           (时间相关实现)
    ├── constants		(常量、错误定义)
    ├── device_interface(设备接口定义)
├── tests                   (测试程序)
├── tools                   (一些dts文件)
└── userlibc                (rust lib库)
```



## DBFS2 Integration（数据库文件系统）

这个 fork 把 **DBFS2**（用 JammDB 做 KV 存储的数据库文件系统）接进了 Alien，在 QEMU 里
**能挂载到 `/dbfs`**。boot 的时候内核会打印：

```text
[dbfs] step1: lookup /dev/dbfs ...
[dbfs] step2: /dev/dbfs FOUND
[dbfs] step3: got dev inode, i_mount ...
[dbfs] step4: i_mount OK
[dbfs] step5: mounted at /dbfs
mount fs success
```

挂上之后 `/dbfs` 能读写、能建目录，实测记录在
`_dbfs2_alien/docs/HOW-DBFS2-WAS-INTEGRATED.md` 第 6 节。

存储后端走的是**同步**的路子：整库以内存镜像 + write-through 的方式坐在块设备上，
没有引入 future，也没上绿色线程（为什么这么做，那份文档的第 3.5 节写了六条理由）。

### 分层结构

```text
用户态        busybox / pjdfstest / dbfs_functest
                 │  syscall (open / read / write / rename …)
                 ▼
内核 VFS      subsystems/vfs   →   vfscore（trait 式 API）
                 │  实现 VfsInode / VfsFile / VfsFsType / VfsDentry
                 ▼
适配层        _dbfs2_alien/adapter/      ← 新建，薄适配层
                 │  dbfs_common_*(ino: usize, buf: &[u8], …)
                 ▼
DBFS2 核心    _dbfs2_alien/dbfs2/        文件/目录存成 KV 记录
                 │
                 ▼
JammDB        _dbfs2_alien/vendor/jammdb-patched/
                 │  read_at / write_at
                 ▼
块设备层      /dev/dbfs        RAMDISK 或 virtio-blk
```

有一点要说明：DBFS2 是**坐在块设备层之上**的，不是自己 malloc 一块内存当磁盘，
页读写全部经由 `VfsInode::read_at / write_at` 落到 Alien 的块设备，跟 fat32 一个路子。

### 相关文档

| 文档 | 内容 |
|---|---|
| `_dbfs2_alien/docs/HOW-DBFS2-WAS-INTEGRATED.md` | 怎么接的、内部结构、接口怎么兼容的、怎么证明挂载成功 |
| `docs/ENV-SETUP-QA-2026-09-15.md` | 编译环境搭建问答日志 |

> 挂载成功不等于功能完备。pjdfstest 在 `/dbfs` 上还有失败项；针对已发现缺陷的补丁
> **没在当前仓库验证过**（所以不算成果，这里也不列）；性能和崩溃一致性没有做过完整对照测试。
> 详细清单在上面第一份文档的第 7 节。

## Run

1. install qemu 7.0.0(qume版本最低要求7.0.0)
2. install rust nightly
3. install riscv64-linux-musl [toolchain](https://musl.cc/)<br>
以上内容可以参考[简明 ArceOS Tutorial Book](https://rcore-os.cn/arceos-tutorial-book/ch01-02.html)

```
make help
```

```
# 一键运行qemu，注意在编译busybox时选择静态链接Settings->Build static binary (no shared libs)
# 忘记设置静态链接可以使用make clean重新配置
make run
# run test
> cd tests
> ./final_test
```

### Run with GUI (QEMU)

```
make run GUI=y
# 在编译和运行的时候指定参数y
cd tests
slint or sysinfo or todo or printdemo or memorygame 
```

### [Run VisionFive2](./docs/doc/boot.md)

Update the `TFTPBOOT`  variable in Makefile.

```
make sdcard
make vf2 VF2=y SMP=2
// 生成testos.bin
// 这里smp=2 表示的是单核启动，对于u74-mc处理器，0号核不会被启动，从1号开始。
```

## GDB

1. `make gdb-server`
2. `make gdb-client`

## [Doc](https://godones.github.io/Alien/)



## Reference

- rCoreTutorial-v3 http://rcore-os.cn/rCore-Tutorial-Book-v3/chapter0/index.html
- Maturin https://gitlab.eduxiji.net/scPointer/maturin
- Redox https://gitlab.redox-os.org/redox-os/
- [Files · master · FTL OS / OSKernel2022-FTLOS · GitLab (eduxiji.net)](https://gitlab.eduxiji.net/DarkAngelEX/oskernel2022-ftlos/-/tree/master)

