"""Spike server: static page, public CRS slices, auction terms, and a /submit that
stores what the page posted, as the operators would receive it. Logs every request.
Usage: python3 -I serve.py <site> <crs-dir> <grumpkin-file> <auction.json> <receive-dir> <port> [--isolated]"""
import base64
import json
import os
import pathlib
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

site, crs, grumpkin, auction, receive, port = sys.argv[1:7]
isolated = '--isolated' in sys.argv
site, receive = pathlib.Path(site), pathlib.Path(receive)
terms = json.loads(pathlib.Path(auction).read_text())
TYPES = {'.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm'}


class Handler(BaseHTTPRequestHandler):
    def send(self, status, body, kind='application/octet-stream'):
        self.send_response(status)
        self.send_header('Content-Type', kind)
        self.send_header('Content-Length', str(len(body)))
        if isolated:
            self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
            self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = self.path.split('?')[0].lstrip('/') or 'index.html'
        if path == 'auction.json':
            return self.send(200, json.dumps(terms).encode(), 'application/json')
        if path == 'crs/g1':
            with open(os.path.join(crs, 'bn254_g1.dat'), 'rb') as f:
                return self.send(200, f.read(terms['srs_points'] * 64))
        if path == 'crs/g2':
            return self.send(200, pathlib.Path(crs, 'bn254_g2.dat').read_bytes())
        if path == 'crs/grumpkin':
            return self.send(200, pathlib.Path(grumpkin).read_bytes())
        file = (site / path).resolve()
        if not file.is_relative_to(site.resolve()) or not file.is_file():
            return self.send(404, b'')
        self.send(200, file.read_bytes(), TYPES.get(file.suffix, 'application/octet-stream'))

    def do_POST(self):
        if self.path != '/submit':
            return self.send(404, b'')
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        receive.mkdir(mode=0o700, exist_ok=False)
        i = body['book']['active'].index(True)
        files = {'book.json': json.dumps(body['book']).encode(), 'proof_plaindriver': base64.b64decode(body['proof']),
                 'public_inputs_plaindriver': base64.b64decode(body['public_inputs'])}
        files.update({f'bidder{i}.toml.{p}.shared': base64.b64decode(v) for p, v in enumerate(body['share_pairs'])})
        for name, data in files.items():
            fd = os.open(receive / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, 'wb') as f:
                f.write(data)
        self.send(200, b'{}', 'application/json')

    def log_message(self, fmt, *args):
        print(json.dumps({'method': self.command, 'path': self.path, 'status': args[1] if len(args) > 1 else None}), flush=True)


ThreadingHTTPServer(('127.0.0.1', int(port)), Handler).serve_forever()
