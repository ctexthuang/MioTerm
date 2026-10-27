import base64
import json
import os
import stat
import sys


def encoded(path):
    return base64.b64encode(path).decode("ascii")


try:
    request = json.load(sys.stdin)
    path = request["path"]
    path = os.fsencode(os.path.expanduser("~")) if path is None else base64.b64decode(path, validate=True)
    path = os.path.abspath(os.path.expanduser(path))
    entries = []
    with os.scandir(path) as listing:
        for entry in listing:
            if len(entries) >= 10000:
                raise ValueError("directory exceeds 10000 entries")
            info = entry.stat(follow_symlinks=False)
            link = stat.S_ISLNK(info.st_mode)
            entries.append({
                "name": encoded(entry.name),
                "directory": entry.is_dir(follow_symlinks=True),
                "regular": stat.S_ISREG(info.st_mode),
                "link": link,
                "size": info.st_size,
            })
    print(json.dumps({
        "path": encoded(path),
        "parent": encoded(os.path.dirname(path)),
        "entries": entries,
    }))
except Exception as error:
    print("remote files: " + str(error), file=sys.stderr)
    sys.exit(1)
