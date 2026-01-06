#!/usr/bin/env python3
"""
Mock Kernel TCP Server for Elle DBFS Testing
Simulates the Alien kernel's TCP Server for testing Host client
"""

import socket
import struct
import json
from typing import Tuple, Optional

# ========================================
# Bincode compatible serialization (simplified)
# ========================================

def serialize_request(tx_id: int, op_type: int, path: str, offset: int, data: bytes) -> bytes:
    """Simplified serialization of DbfsRequest"""
    # Format: tx_id(8) + op_type(1) + path_len(4) + path + offset(8) + data_len(4) + data
    path_bytes = path.encode('utf-8')
    result = struct.pack('>QBI', tx_id, op_type, len(path_bytes))
    result += path_bytes
    result += struct.pack('>Q', offset)
    result += struct.pack('>I', len(data))
    result += data
    return result

def deserialize_response(data: bytes) -> dict:
    """Simplified deserialization of DbfsResponse"""
    # Format: tx_id(8) + status(4) + lsn(8) + data_len(4) + data
    if len(data) < 24:
        raise ValueError(f"Response too short: {len(data)}")

    tx_id, status, lsn, data_len = struct.unpack('>QIIQ', data[:24])
    resp_data = data[24:24+data_len] if data_len > 0 else b''

    return {
        'tx_id': tx_id,
        'status': status,
        'lsn': lsn,
        'data': resp_data
    }

def parse_request(data: bytes) -> dict:
    """Parse incoming DbfsRequest from Host client"""
    if len(data) < 25:
        raise ValueError(f"Request too short: {len(data)}")

    tx_id, op_type, path_len = struct.unpack('>QBI', data[:13])
    offset = 0
    data_content = b''

    if len(data) >= 13 + path_len:
        path = data[13:13+path_len].decode('utf-8', errors='ignore')
        remaining = data[13+path_len:]

        if len(remaining) >= 8:
            offset = struct.unpack('>Q', remaining[:8])[0]
            remaining = remaining[8:]

            if len(remaining) >= 4:
                data_len = struct.unpack('>I', remaining[:4])[0]
                data_content = remaining[4:4+data_len] if data_len > 0 else b''

    return {
        'tx_id': tx_id,
        'op_type': op_type,
        'path': path,
        'offset': offset,
        'data': data_content
    }

def serialize_response(tx_id: int, status: int, lsn: int, data: bytes = b'') -> bytes:
    """Serialize DbfsResponse to send back to Host"""
    result = struct.pack('>QIIQ', tx_id, status, lsn, len(data))
    result += data
    return result

# ========================================
# Operation handlers
# ========================================

OP_NAMES = {
    1: "BeginTx",
    2: "WriteFile",
    3: "CreateFile",
    4: "DeleteFile",
    5: "Mkdir",
    6: "Readdir",
    7: "CommitTx",
    8: "RollbackTx"
}

class MockDBFS:
    """Mock DBFS transactional filesystem"""

    def __init__(self):
        self.next_lsn = 1
        self.files = {}  # path -> data
        self.transactions = {}  # tx_id -> {lsn, operations}

    def begin_tx(self, tx_id: int) -> int:
        """Begin a new transaction"""
        lsn = self.next_lsn
        self.next_lsn += 1
        self.transactions[tx_id] = {
            'lsn': lsn,
            'operations': []
        }
        print(f"  TX-{tx_id}: BEGIN -> LSN={lsn}")
        return lsn

    def commit_tx(self, tx_id: int) -> int:
        """Commit a transaction"""
        if tx_id in self.transactions:
            lsn = self.transactions[tx_id]['lsn']
            print(f"  TX-{tx_id}: COMMIT -> LSN={lsn}")
            del self.transactions[tx_id]
            return lsn
        return 0

    def rollback_tx(self, tx_id: int):
        """Rollback a transaction"""
        print(f"  TX-{tx_id}: ROLLBACK")
        if tx_id in self.transactions:
            del self.transactions[tx_id]

    def write_file(self, tx_id: int, path: str, offset: int, data: bytes):
        """Write to a file"""
        print(f"  TX-{tx_id}: WRITE {path} @{offset} ({len(data)} bytes)")

    def create_file(self, tx_id: int, path: str):
        """Create a new file"""
        print(f"  TX-{tx_id}: CREATE {path}")
        self.files[path] = b''

    def delete_file(self, tx_id: int, path: str):
        """Delete a file"""
        print(f"  TX-{tx_id}: DELETE {path}")
        if path in self.files:
            del self.files[path]

    def mkdir(self, tx_id: int, path: str):
        """Create directory"""
        print(f"  TX-{tx_id}: MKDIR {path}")

    def readdir(self, tx_id: int, path: str) -> bytes:
        """Read directory"""
        print(f"  TX-{tx_id}: READDIR {path}")
        # Return JSON array of files
        files_list = list(self.files.keys())
        return json.dumps(files_list).encode('utf-8')

# ========================================
# TCP Server
# ========================================

def handle_client(conn: socket.socket, addr: Tuple[str, int], dbfs: MockDBFS):
    """Handle a single client connection"""
    print(f"📨 New connection from {addr}")

    try:
        while True:
            # 1. Read length prefix (4 bytes big-endian)
            len_data = conn.recv(4)
            if not len_data:
                break

            req_len = struct.unpack('>I', len_data)[0]
            print(f"📦 Receiving {req_len} bytes")

            # 2. Read request data
            req_data = b''
            while len(req_data) < req_len:
                chunk = conn.recv(req_len - len(req_data))
                if not chunk:
                    raise ConnectionError("Connection closed prematurely")
                req_data += chunk

            # 3. Parse request
            try:
                req = parse_request(req_data)
                op_name = OP_NAMES.get(req['op_type'], f"Unknown({req['op_type']})")
                print(f"📨 TX-{req['tx_id']}: {op_name}")

                # 4. Handle operation
                status = 0  # Success
                lsn = 0
                resp_data = b''

                op = req['op_type']
                tx_id = req['tx_id']

                if op == 1:  # BeginTx
                    lsn = dbfs.begin_tx(tx_id)

                elif op == 2:  # WriteFile
                    dbfs.write_file(tx_id, req['path'], req['offset'], req['data'])

                elif op == 3:  # CreateFile
                    dbfs.create_file(tx_id, req['path'])

                elif op == 4:  # DeleteFile
                    dbfs.delete_file(tx_id, req['path'])

                elif op == 5:  # Mkdir
                    dbfs.mkdir(tx_id, req['path'])

                elif op == 6:  # Readdir
                    resp_data = dbfs.readdir(tx_id, req['path'])

                elif op == 7:  # CommitTx
                    lsn = dbfs.commit_tx(tx_id)

                elif op == 8:  # RollbackTx
                    dbfs.rollback_tx(tx_id)

                # 5. Send response
                resp = serialize_response(tx_id, status, lsn, resp_data)
                resp_len = struct.pack('>I', len(resp))

                conn.sendall(resp_len + resp)
                print(f"📤 Sent {len(resp)} bytes")

            except Exception as e:
                print(f"❌ Error handling request: {e}")
                # Send error response
                error_resp = serialize_response(0, -1, 0)
                conn.sendall(struct.pack('>I', len(error_resp)) + error_resp)
                break

    except ConnectionResetError:
        print("Connection reset by client")
    except Exception as e:
        print(f"❌ Connection error: {e}")
    finally:
        conn.close()
        print("✅ Connection closed")

def run_server(port: int = 12345):
    """Run the mock kernel TCP server"""
    dbfs = MockDBFS()

    print("========================================")
    print("🚀 Mock Kernel TCP Server")
    print("========================================")
    print(f"Port: {port}")
    print(f"Mode: Mock DBFS operations")
    print(f"Protocol: Length-prefixed binary")
    print("========================================")

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        s.bind(('0.0.0.0', port))
        s.listen(5)

        print(f"✅ Server listening on 0.0.0.0:{port}")
        print("")
        print("Ready to accept Elle test clients from Host")
        print("")

        conn_count = 0
        while True:
            conn, addr = s.accept()
            conn_count += 1
            print(f"========================================")
            print(f"Connection #{conn_count} from {addr}")
            print(f"========================================")

            handle_client(conn, addr, dbfs)

            print("")

if __name__ == '__main__':
    import sys
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 12345

    try:
        run_server(port)
    except KeyboardInterrupt:
        print("\n\n⏹️  Server stopped by user")
