#!/usr/bin/env python

import sys
from pathlib import Path

from jinja2 import DictLoader, Environment, select_autoescape


def main():
    if len(sys.argv) != 2:
        print(f"USAGE: {sys.argv[0]} <template path>", file=sys.stderr)
        sys.exit(1)

    template = Path(sys.argv[1]).read_text()
    jinja_env = Environment(
        loader=DictLoader({"index": template}),
        autoescape=select_autoescape(default=False, default_for_string=False),
    )
    template = jinja_env.get_template("index")
    print(template.render(dict()))


if __name__ == "__main__":
    main()
