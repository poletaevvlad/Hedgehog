installdir =
pkgname = hedgehog
features = --features mpris

target/release/hedgehog: $(shell find hedgehog-tui hedgehog-player hedgehog-library -name '*.rs') Cargo.toml Cargo.lock $(wildcard */Cargo.toml)
	cargo build --release $(features) --message-format=json-render-diagnostics \
		| jq -r "select(.out_dir) | select(.package_id | startswith(\"hedgehog-tui \")) | .out_dir" \
		> ./target/out_dir_path

.PHONY: install
install: target/release/hedgehog hedgehog.1
	find $(shell cat ./target/out_dir_path) -type f -exec install -Dm644 "{}" "$(installdir)/usr/share/hedgehog" \;
	install -Dm644 "./LICENSE" "$(installdir)/usr/share/licenses/$(pkgname)/LICENSE"
	install -Dm644 "./hedgehog.1" "$(installdir)/usr/share/man/man1/hedgehog.1"
	install -Dm755 "./target/release/hedgehog" "$(installdir)/usr/bin/hedgehog"

hedgehog.1: hedgehog.1.ronn
	ronn -r --pipe hedgehog.1.ronn \
		| sed 's/.IP "\\\[ci\]" 4/.IP "\\\[bu\]" 2/g' \
		> hedgehog.1

.PHONY: man
man: hedgehog.1
	man -l ./hedgehog.1
