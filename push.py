#! /usr/bin/env python3

from duct import cmd, sh
import os
from pathlib import Path
import sys

here = Path(__file__).parent


def main():
    os.chdir(str(here))

    status = sh("git status --porcelain").read()
    if status:
        print("repo isn't clean")
        return 1

    sh("git fetch").run()

    commits = sh("git log origin/master..").read()
    if commits:
        print("unpushed commits")
        return 1

    cmd("ssh", "jacko@jacko.io", "cd /srv/jacko.io && git pull --ff-only").run()


if __name__ == "__main__":
    sys.exit(main())
