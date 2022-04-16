#!/bin/sh

if [ -z $PREFIX ]; then
    PREFIX=/usr
fi

rm -rf "$PREFIX/share/hedgehog"
rm -rf "$PREFIX/share/licenses/hedgehog"
rm "$PREFIX/share/man/man1/hedgehog.1"
rm "$PREFIX/bin/hedgehog"
