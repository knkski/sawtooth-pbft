from http.server import BaseHTTPRequestHandler, HTTPServer

FIRST = None


class Bootstrap(BaseHTTPRequestHandler):
    def __init__(self, *args):
        BaseHTTPRequestHandler.__init__(self, *args)

    def do_POST(self):
        global FIRST
        if FIRST is None:
            FIRST = self.rfile.read(int(self.headers['Content-Length']))

        print("Got POST request, first responder is: %s" % FIRST)
        self.send_response(200)
        self.send_header('Content-type', 'text/plain')
        self.end_headers()
        self.wfile.write(FIRST)


if __name__ == '__main__':
    httpd = HTTPServer(('', 8000), Bootstrap)
    print("Counter is ready.")

    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass

    httpd.server_close()
