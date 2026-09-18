import socket
import json
import time

def send_request(sock, method, params, id=1):
    req = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id
    }
    payload = json.dumps(req).encode('utf-8')
    sock.sendall(payload)
    # Give the server a tiny bit of time to respond, in a real app we'd parse JSON stream
    time.sleep(0.1)
    try:
        resp = sock.recv(4096)
        print("Response:", resp.decode('utf-8'))
    except socket.error as e:
        print("Socket error:", e)

def main():
    sock_path = "/tmp/well-caduceus.sock"
    
    print(f"Connecting to {sock_path}...")
    try:
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect(sock_path)
    except Exception as e:
        print(f"Failed to connect: {e}")
        return

    print("Sending ai.session_start...")
    send_request(s, "ai.session_start", {
        "session_id": "test_123",
        "agent_name": "TestAgent"
    })
    
    print("Sending ai.tool_use...")
    send_request(s, "ai.tool_use", {
        "tool_name": "grep_search",
        "query": "something"
    })
    
    print("Sending ai.stream_diff with blocklist...")
    send_request(s, "ai.stream_diff", {
        "blocks": [
            {
                "type": "text",
                "content": "Here is the code you requested:"
            },
            {
                "type": "code",
                "lang": "rust",
                "content": "fn main() {\n    println!(\"Hello, world!\");\n}"
            }
        ]
    })
    
    print("Closing connection.")
    s.close()

if __name__ == "__main__":
    main()
