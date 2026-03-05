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

SSH_BASH = """
set -e -u -o pipefail

cd /srv/jacko.io

if [[ -n "$(git status --porcelain --untracked-files=no)" ]] ; then
    echo "server repo is dirty"
    exit 1
fi

git fetch --all
git reset --hard origin/master
peru sync --no-cache
(cd render_posts && cargo run --release)
"""


def main():
    os.chdir(str(here))

    status = cmd("git", "status", "--porcelain", "--untracked-files=no").read()
    if status:
        print("local repo is dirty")
        return 1

    cmd("git", "push", "origin", "master").run()

    output = cmd("ssh", "jacko@jacko.io", "/usr/bin/bash").stdin_bytes(SSH_BASH).unchecked().run()
    if output.status != 0:
        return 1


if __name__ == "__main__":
    sys.exit(main())
