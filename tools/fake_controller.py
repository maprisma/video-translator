"""Test-only stand-in for the caption controller: answers overlay-state on the controller's
socket with fixed captions and prints what the shell sends (luminance, geometry)."""
import json
import os
import socket
import sys

path = os.path.join(os.environ['XDG_RUNTIME_DIR'], f'video-trans-{os.getuid()}.sock')
text_color = sys.argv[1] if len(sys.argv) > 1 else 'auto'
view = dict(open=True, text='Der schnelle braune Fuchs springt über den faulen Hund.\n\n'
            'Untertitel erscheinen hier, über jedem Video.', status='Listening', caption_received=True,
            theme=dict(background='#000000', foreground='#ffffff', border_color='#3584e4', accent='#3584e4'),
            text_color=text_color, font_size=24, auto_scroll=True, background_transparency=40, border_width=2,
            width=900, height=200)
server = socket.socket(socket.AF_UNIX)
server.bind(path)
server.listen(4)
seen = set()
while True:
    connection, _ = server.accept()
    with connection:
        request = json.loads(connection.makefile().readline())
        for key in ('luminance', 'accent', 'width'):
            if key in request and (key, request.get(key)) not in seen:
                seen.add((key, request.get(key)))
                print(f'[fake controller] {request["action"]} {key}={request[key]}', flush=True)
        connection.sendall((json.dumps(view if request['action'] == 'overlay-state' else {}) + '\n').encode())
