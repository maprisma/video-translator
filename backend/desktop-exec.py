#!/usr/bin/env python3
"""Replace a desktop entry's executable with an absolute, desktop-spec quoted path.

Apply Exec argument quoting first, then the desktop value escaping layer. Percent
is a field code even inside quotes, so a literal percent must be doubled.
"""
import sys
from pathlib import Path


def quote_executable(path):
    argument = ''.join('\\' + c if c in '\\"`$' else c for c in str(path))
    return '"' + argument.replace('\\', '\\\\').replace('%', '%%') + '"'


def render(template, executable):
    lines = []
    for line in template.splitlines(keepends=True):
        if line.startswith('Exec='):
            # Templates use a bare executable followed by optional action arguments.
            rest = line[5:].rstrip('\n').split(' ', 1)
            line = 'Exec=' + quote_executable(executable) + (' ' + rest[1] if len(rest) > 1 else '') + '\n'
        lines.append(line)
    return ''.join(lines)


if __name__ == '__main__':
    sys.stdout.write(render(Path(sys.argv[1]).read_text(), sys.argv[2]))
