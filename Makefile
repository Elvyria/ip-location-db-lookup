.DEFAULT_GOAL := release

prefix ?= /usr/local
bindir ?= ${prefix}/bin

name = $(shell sed -nE 's/name *?= *?"(.+)"/\1/p' ./Cargo.toml)
ifdef CARGO_TARGET_DIR
	target = ${CARGO_TARGET_DIR}
else
	target = ./target
endif

release:
	$(MAKE) clean
	cargo build --locked --release

debug:
	cargo build --locked

clean:
	cargo clean --package ${name}

install:
	test -d ${target}/release
	install -m 0755 -s ${target}/release/${name} ${bindir}/${name}

uninstall:
	rm ${bindir}/${name}
