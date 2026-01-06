#!/usr/bin/env python3
"""
Simple TCP Echo Server for Basic Testing
Just echoes back success responses without parsing
"""

import socket
import struct
import time

def run_server(port: int = 12345):
    """Run a simple echo server"""

    print("========================================")
    print("🚀 Simple Test TCP Server")
    print("========================================")
    print(f"Port: {port}")
    print(f"Mode: Echo (responds with success)")
    print("========================================")

    conn_count = 0

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind(('0.0.0.0', port))
        s.listen(5)

        print(f"✅ Server listening on 0.0.0.0:{port}")
        print("")
        print("Ready to accept connections")
        print("")

        while True:
            conn, addr = s.accept()
            conn_count += 1
            print(f"========================================")
            print(f"Connection #{conn_count} from {addr}")
            print(f"========================================")

            try:
                conn.settimeout(60.0)  # 60 秒超时
                req_count = 0

                while True:
                    try:
                        # Read length prefix (4 bytes big-endian)
                        len_data = conn.recv(4)
                        if not len_data:
                            print("Connection closed by client")
                            break

                        req_len = struct.unpack('>I', len_data)[0]
                        req_count += 1
                        print(f"📦 [{req_count}] Receiving {req_len} bytes")

                        # Read request data
                        req_data = b''
                        while len(req_data) < req_len:
                            chunk = conn.recv(req_len - len(req_data))
                            if not chunk:
                                raise ConnectionError("Connection closed")
                            req_data += chunk

                        print(f"📨 [{req_count}] Received {len(req_data)} bytes")

                        # Parse tx_id from bincode (first 8 bytes)
                        tx_id = struct.unpack('>Q', req_data[:8])[0]
                        print(f"   TX-ID: {tx_id}")

                        # Send success response
                        # Format: tx_id(8) + status(4) + lsn(8) + data_len(4) + data
                        lsn = req_count
                        resp = struct.pack('>QIIQ', tx_id, 0, lsn, 0)
                        resp_len = struct.pack('>I', len(resp))

                        conn.sendall(resp_len + resp)
                        print(f"📤 [{req_count}] Sent {len(resp)} bytes (status=0, lsn={lsn})")

                    except socket.timeout:
                        print("⏱️  Connection timeout (60s)")
                        break
                    except Exception as e:
                        print(f"❌ Request error: {e}")
                        break

                print(f"✅ Connection processed {req_count} requests")

            except Exception as e:
                print(f"❌ Connection error: {e}")
            finally:
                conn.close()
                print("✅ Connection closed")
                print("")

if __name__ == '__main__':
    import sys
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 12345

    try:
        run_server(port)
    except KeyboardInterrupt:
        print("\n\n⏹️  Server stopped")
