#! /usr/bin/env python3

# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "duct",
# ]
# ///

from duct import cmd
import os
from pathlib import Path
import sys

here = Path(__file__).parent


def main():
    os.chdir(str(here))

    status = cmd("git", "status", "--porcelain").read()
    if status:
        print("repo isn't clean")
        return 1

    cmd("git", "push", "origin", "master").run()

    cmd(
        "ssh",
        "jacko@jacko.io",
        "cd /srv/jacko.io && git pull --ff-only && peru sync --no-cache && (cd render_posts && cargo run --release)",
    ).run()


if __name__ == "__main__":
    sys.exit(main())
