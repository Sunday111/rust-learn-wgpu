import http.server
import socketserver
from pathlib import Path

PORT = 8000

SCRIPT_PATH = Path(__file__)
SCRIPT_DIR = SCRIPT_PATH.parent.resolve()
SERVE_DIR = SCRIPT_DIR/'wasm-build'

def handler(*args, **kwargs):
    return http.server.SimpleHTTPRequestHandler(*args, directory=SERVE_DIR, **kwargs)

def main():
    try:
        with socketserver.TCPServer(("", PORT), handler) as httpd:
            print(f"Serving {SERVE_DIR.relative_to(SCRIPT_DIR)} at http://localhost:{PORT}")
            httpd.serve_forever()
    except KeyboardInterrupt:
        print('Stopped with keyboard interrupt')

if __name__ == '__main__':
    main()
