from http.server import HTTPServer, SimpleHTTPRequestHandler
import cgi
import os

FIRMWARE_FILE = "firmware.bin"
VERSION_FILE = "version.txt"

# Ensure version.txt exists
if not os.path.exists(VERSION_FILE):
    with open(VERSION_FILE, "w") as f:
        f.write("0.0.0")  # initial version

UPLOAD_FORM = b"""
<!DOCTYPE html>
<html>
<head>
    <title>ESP32 OTA Upload</title>
</head>
<body>
    <h1>Upload New Firmware</h1>
    <form enctype="multipart/form-data" method="post" action="/upload">
        <label>Firmware file:</label>
        <input type="file" name="firmware"><br><br>
        <label>Version:</label>
        <input type="text" name="version" placeholder="e.g., 1.0.0"><br><br>
        <input type="submit" value="Upload">
    </form>
    <br>
    <form method="post" action="/delete">
        <input type="submit" value="Delete Firmware">
    </form>
</body>
</html>
"""

class OTAHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/version.txt":
            # Serve version
            with open(VERSION_FILE, "r") as f:
                version = f.read().strip()
            self.send_response(200)
            self.send_header("Content-type", "text/plain")
            self.end_headers()
            self.wfile.write(version.encode())
        elif self.path == "/firmware.bin":
            # Serve firmware binary
            if os.path.exists(FIRMWARE_FILE):
                self.send_response(200)
                self.send_header("Content-type", "application/octet-stream")
                self.end_headers()
                with open(FIRMWARE_FILE, "rb") as f:
                    self.wfile.write(f.read())
            else:
                self.send_response(404)
                self.end_headers()
        else:
            # Serve the upload form at root
            self.send_response(200)
            self.send_header("Content-type", "text/html")
            self.end_headers()
            self.wfile.write(UPLOAD_FORM)

    def do_POST(self):
        if self.path == "/upload":
            content_type = self.headers.get("Content-Type")
            if not content_type:
                self.send_response(400)
                self.end_headers()
                return

            form = cgi.FieldStorage(
                fp=self.rfile,
                headers=self.headers,
                environ={"REQUEST_METHOD":"POST",
                         "CONTENT_TYPE":content_type}
            )
            fileitem = form['firmware']
            version = form.getvalue("version", "0.0.0")

            if fileitem.file:
                # Save firmware directly in root
                with open(FIRMWARE_FILE, "wb") as f:
                    f.write(fileitem.file.read())
                # Save version
                with open(VERSION_FILE, "w") as f:
                    f.write(version)

                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"Firmware uploaded!")
            else:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"No firmware file received.")

        elif self.path == "/delete":
            # Delete firmware
            if os.path.exists(FIRMWARE_FILE):
                os.remove(FIRMWARE_FILE)
            # Reset version
            with open(VERSION_FILE, "w") as f:
                f.write("0.0.0")
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"Firmware deleted and version reset.")

def run(server_class=HTTPServer, handler_class=OTAHandler, port=8080):
    server_address = ("", port)
    httpd = server_class(server_address, handler_class)
    print(f"OTA Server running on port {port}")
    httpd.serve_forever()

if __name__ == "__main__":
    run()
