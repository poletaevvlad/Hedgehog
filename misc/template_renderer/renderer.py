#!/usr/bin/env python

import sys
from pathlib import Path
from typing import Tuple

import toml
from jinja2 import Environment, FileSystemLoader, select_autoescape


def parse_color(color: str) -> Tuple[int, int, int]:
    color_int = int(color, 16)
    return (color_int >> 16) & 0xFF, (color_int >> 8) & 0xFF, color_int & 0xFF


def color_mix(fg: str, bg: str, factor: float) -> str:
    fg_channels = parse_color(fg)
    bg_channels = parse_color(bg)
    red, green, blue = (
        int(ch1 * factor + ch2 * (1 - factor))
        for ch1, ch2 in zip(bg_channels, fg_channels)
    )
    return f"{red:02x}{green:02x}{blue:02x}"


def main():
    if len(sys.argv) < 2:
        print(f"USAGE: {sys.argv[0]} (<var>=<value>)* <template path>", file=sys.stderr)
        sys.exit(1)

    path = Path(sys.argv[-1]).resolve()
    jinja_env = Environment(
        loader=FileSystemLoader(str(path.parent)),
        autoescape=select_autoescape(default=False, default_for_string=False),
    )
    jinja_env.globals["color_mix"] = color_mix

    template = jinja_env.get_template(path.name)

    context = toml.load(sys.stdin)
    for values in sys.argv[1:-1]:
        key_value = values.split("=", maxsplit=2)
        if len(key_value) != 2:
            print(f"Invalid key-value pair '{key_value}'\n", file=sys.stderr)
            print(
                f"USAGE: {sys.argv[0]} (<var>=<value>)* <template path>",
                file=sys.stderr,
            )
            sys.exit(1)
        key, value = key_value
        context[key] = value

    repo_root = path.parent
    while repo_root != repo_root.root:
        if (repo_root / ".git").is_dir():
            break
        repo_root = repo_root.parent
    context["_template"] = path.relative_to(repo_root)

    print(template.render(context))


if __name__ == "__main__":
    main()
