#!/bin/sh

AROOT=$(dirname $0)

if [ -z $PREFIX ]; then
    PREFIX=/usr
fi

rm -rf "$PREFIX/share/hedgehog"
rm -rf "$PREFIX/share/licenses/hedgehog"
install -Dm644 "$AROOT/LICENSE" "$PREFIX/share/licenses/hedgehog/LICENSE"
install -Dm644 "$AROOT/hedgehog.1" "$PREFIX/share/man/man1/hedgehog.1"
install -Dm755 "$AROOT/hedgehog" "$PREFIX/bin/hedgehog"
install -Dm755 "$AROOT/uninstall.sh" "$PREFIX/share/hedgehog/uninstall.sh"

cd "$AROOT/usr/share/hedgehog"
find . -type f -exec install -Dm644 "{}" "$PREFIX/share/hedgehog" \;
