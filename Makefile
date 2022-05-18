installdir =
pkgname = hedgehog
features = --features mpris

target/release/hedgehog: $(shell find hedgehog-tui hedgehog-player hedgehog-library -name '*.rs') Cargo.toml Cargo.lock $(wildcard */Cargo.toml)
	mkdir -p ./target
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

hedgehog.1.html: hedgehog.1.ronn
	ronn hedgehog.1.ronn -5 --style toc,dark,darktoc

.PHONY: man
man: hedgehog.1
	man -l ./hedgehog.1

.PHONY: version
version:
	@cat ./hedgehog-tui/Cargo.toml | grep -P '^version = ' | grep -oP '\d+\.\d+\.\d+' 

.PHONY: archive
archive: target/release/hedgehog
	$(eval archive_name := hedgehog-$(shell make version)-$(shell uname -s | sed -E 's/[[:upper:]]/\L\0/g')-$(shell uname -m))
	mkdir -p ./build
	if [ -d ./build/$(archive_name) ]; then rm -r ./build/$(archive_name); fi
	mkdir ./build/$(archive_name)
	cp ./LICENSE ./hedgehog.1 ./build/$(archive_name)
	cp ./assets/archive-install.sh ./build/$(archive_name)/install.sh
	cp ./assets/archive-uninstall.sh ./build/$(archive_name)/uninstall.sh
	chmod +x ./build/$(archive_name)/install.sh ./build/$(archive_name)/uninstall.sh
	cp ./target/release/hedgehog ./build/$(archive_name)/hedgehog
	strip ./build/$(archive_name)/hedgehog
	mkdir -p ./build/$(archive_name)/usr/share
	cp -r $(shell cat ./target/out_dir_path)/config ./build/$(archive_name)/usr/share/hedgehog
	cd ./build/ && tar -czvf $(archive_name).tar.gz $(archive_name)
	rm -r ./build/$(archive_name)
