import json
import os
import pathlib
import pwd
import subprocess
import sys
import tempfile


def main():
    scripts = json.loads(sys.argv[1])
    shell = os.environ.get("SHELL") or pwd.getpwuid(os.getuid()).pw_shell
    name = pathlib.Path(shell).name
    if name not in ("zsh", "bash", "fish"):
        raise RuntimeError("directory reporting requires a zsh, bash or fish login shell: " + shell)
    with tempfile.TemporaryDirectory(prefix="elyterminal-shell-") as directory:
        root = pathlib.Path(directory)
        for relative, text in scripts.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        env = os.environ.copy()
        args = [shell]
        if name == "zsh":
            if "ZDOTDIR" in env:
                env["ELY_ZSH_ZDOTDIR"] = env["ZDOTDIR"]
            env["ZDOTDIR"] = str(root / "zsh")
            args += ["-il"]
        elif name == "bash":
            env["ELY_BASH_LOGIN"] = "1"
            args += ["--rcfile", str(root / "bash/elyterminal.bash"), "-i"]
        else:
            previous = env.get("XDG_DATA_DIRS", "")
            env["ELY_FISH_XDG_DATA_DIRS"] = previous
            env["XDG_DATA_DIRS"] = directory + ":" + (previous or "/usr/local/share:/usr/share")
            args += ["-il"]
        return subprocess.call(args, env=env)


try:
    sys.exit(main())
except Exception as error:
    print("ssh: remote shell integration failed: " + str(error), file=sys.stderr)
    sys.exit(1)
