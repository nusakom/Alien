//! TCP Socket Server for Elle + Jepsen Testing
//!
//! 运行在 Alien 内核中,监听 TCP 端口接收来自 Host 客户端的请求
//! 使用与 Host 客户端相同的二进制协议: [4字节长度][bincode数据]

#![allow(dead_code)]

use alloc::{format, string::String, vec::Vec};
use alloc::vec;
use core::net::{IpAddr, SocketAddr};
use log::{info, error, debug};

use crate::elle_protocol::{DbfsRequest, DbfsResponse};
use crate::elle_handler_real::ElleRequestHandlerReal;

// ==================== TCP 封装 ====================

/// TCP 流封装 (类似 std::io::Read/Write)
pub struct TcpStream {
    inner: devices::net::nettest::TcpStream,
}

impl TcpStream {
    /// 读取数据 (填充整个 buffer)
    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), ()> {
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.inner.read(&mut buf[total_read..]) {
                Ok(n) => {
                    if n == 0 {
                        return Err(()); // 连接关闭
                    }
                    total_read += n;
                }
                Err(_) => return Err(()),
            }
        }
        Ok(())
    }

    /// 写入所有数据
    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), ()> {
        self.inner.write_all(buf).map_err(|_| ())?;
        Ok(())
    }
}

/// TCP 监听器封装
pub struct TcpListener {
    inner: devices::net::nettest::TcpListener,
}

impl TcpListener {
    /// 绑定到指定地址
    pub fn bind(addr: SocketAddr) -> Result<Self, ()> {
        let listener = devices::net::nettest::TcpListener::bind(addr)
            .map_err(|_| ())?;
        Ok(Self { inner: listener })
    }

    /// 接受新连接
    pub fn accept(&self) -> Result<(TcpStream, SocketAddr), ()> {
        let (stream, addr) = self.inner.accept().map_err(|_| ())?;
        Ok((TcpStream { inner: stream }, addr))
    }

    /// 获取本地地址
    pub fn local_addr(&self) -> Result<SocketAddr, ()> {
        self.inner.local_addr().map_err(|_| ())
    }
}

// ==================== Elle TCP Server ====================

/// Elle TCP Server
///
/// 接收来自 Host 的 Elle 测试请求并调用真实的 DBFS 操作
pub struct ElleTcpServer {
    addr: SocketAddr,
    handler: ElleRequestHandlerReal,
}

impl ElleTcpServer {
    /// 创建新的 TCP server
    pub fn new(port: u16) -> Self {
        let ip = IpAddr::V4("0.0.0.0".parse().unwrap());
        let addr = SocketAddr::new(ip, port);

        info!("🎯 Elle TCP Server initializing on {}", addr);

        Self {
            addr,
            handler: ElleRequestHandlerReal::new(),
        }
    }

    /// 启动服务器 (阻塞运行)
    pub fn run(&mut self) -> ! {
        // 绑定到端口
        let listener = match TcpListener::bind(self.addr) {
            Ok(l) => l,
            Err(_) => {
                error!("❌ Failed to bind to {}", self.addr);
                panic!("Elle TCP Server: bind failed");
            }
        };

        let local_addr = listener.local_addr().unwrap();
        info!("✅ Elle TCP Server listening on {}", local_addr);
        info!("📡 Ready to accept Elle test clients from Host");
        info!("========================================");

        let mut conn_count = 0u64;

        // 主循环: 接受连接并处理
        loop {
            // 1. 接受新连接
            let (mut stream, peer_addr) = match listener.accept() {
                Ok(conn) => conn,
                Err(e) => {
                    error!("❌ Accept failed: {:?}", e);
                    continue;
                }
            };

            conn_count += 1;
            info!("📨 New connection #{} from {}", conn_count, peer_addr);

            // 2. 处理连接 (简化: 单线程处理一个请求)
            match self.handle_connection(&mut stream) {
                Ok(_) => {
                    info!("✅ Connection #{} closed successfully", conn_count);
                }
                Err(e) => {
                    error!("❌ Connection #{} error: {:?}", conn_count, e);
                }
            }
        }
    }

    /// 处理单个连接
    fn handle_connection(&mut self, stream: &mut TcpStream) -> Result<(), ()> {
        let mut req_count = 0u64;

        // 持续接收请求直到连接关闭
        loop {
            // 1. 读取长度前缀 (4 字节 big-endian)
            let mut len_bytes = [0u8; 4];
            if let Err(_) = stream.read_exact(&mut len_bytes) {
                debug!("Connection closed while reading length");
                return Ok(()); // 正常关闭
            }

            let req_len = u32::from_be_bytes(len_bytes) as usize;

            // 防御: 限制请求大小 (最大 10MB)
            if req_len > 10 * 1024 * 1024 {
                error!("❌ Request too large: {} bytes", req_len);
                return Err(()); // 关闭连接
            }

            debug!("📦 Receiving {} bytes", req_len);

            // 2. 读取请求数据
            let mut req_bytes = vec![0u8; req_len];
            if let Err(_) = stream.read_exact(&mut req_bytes) {
                debug!("Connection closed while reading data");
                return Ok(()); // 正常关闭
            }

            req_count += 1;

            // 3. 反序列化请求
            let req = match DbfsRequest::deserialize(&req_bytes) {
                Ok(r) => r,
                Err(e) => {
                    error!("❌ Failed to deserialize request: {:?}", e);
                    return Err(()); // 协议错误,关闭连接
                }
            };

            debug!("📨 TX-{}: {:?}", req.tx_id, req.op_type);

            // 4. 处理请求 (调用真实的 DBFS 操作)
            let resp = self.handler.handle_request(&req);

            // 5. 序列化响应
            let resp_bytes = resp.serialize();

            // 6. 发送响应 (长度前缀 + 数据)
            let resp_len = resp_bytes.len() as u32;
            let len_prefix = resp_len.to_be_bytes();

            if let Err(_) = stream.write_all(&len_prefix) {
                error!("❌ Failed to send response length");
                return Err(());
            }

            if let Err(_) = stream.write_all(&resp_bytes) {
                error!("❌ Failed to send response data");
                return Err(());
            }

            debug!("📤 Sent {} bytes response", resp_bytes.len());

            // 简化: 每个连接只处理一个请求 (Host 客户端每次重新连接)
            // 这样可以避免复杂的连接状态管理
            // 如果需要性能优化,可以改为长连接模式
            break;
        }

        debug!("Connection processed {} requests", req_count);
        Ok(())
    }
}

// ==================== 启动函数 ====================

/// 启动 Elle TCP Server (在内核初始化时调用)
///
/// 这个函数会阻塞当前线程,因此应该在单独的线程中调用
/// 或者作为内核的主事件循环的一部分
pub fn start_elle_tcp_server(port: u16) -> ! {
    info!("========================================");
    info!("🚀 Starting Elle TCP Server");
    info!("Port: {}", port);
    info!("Mode: Real DBFS operations");
    info!("========================================");

    let mut server = ElleTcpServer::new(port);
    server.run(); // 永不返回
}

/// 初始化并显示 TCP Server 信息 (非阻塞)
///
/// 在内核初始化时调用这个函数来显示 TCP Server 的配置信息
/// 实际的服务器启动可以在单独的线程中或者按需启动
pub fn init_elle_tcp_server_info(port: u16) {
    info!("========================================");
    info!("🎯 Elle TCP Server Configuration");
    info!("Port: {}", port);
    info!("Mode: Real DBFS operations");
    info!("Protocol: Length-prefixed bincode");
    info!("Status: Ready to start");
    info!("");
    info!("To start the server, call:");
    info!("  dbfs::tcp_server::start_elle_tcp_server({});", port);
    info!("========================================");
}
