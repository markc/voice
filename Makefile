PREFIX   ?= $(HOME)/.local
BINDIR   ?= $(PREFIX)/bin
CFLAGS   ?= -O2 -Wall -Wextra -Wpedantic
LDFLAGS  ?=

PKG_CFLAGS  := $(shell pkg-config --cflags libei-1.0)
PKG_LDFLAGS := $(shell pkg-config --libs libei-1.0) -lsystemd

.PHONY: all clean install uninstall rust install-rust

all: ei-type

ei-type: ei-type.c
	$(CC) $(CFLAGS) $(PKG_CFLAGS) -o $@ $< $(LDFLAGS) $(PKG_LDFLAGS)

install: ei-type
	install -d $(BINDIR)
	install -m 755 ei-type $(BINDIR)/ei-type

uninstall:
	rm -f $(BINDIR)/ei-type

rust:
	cargo build --release

install-rust: rust
	install -d $(BINDIR)
	install -m 755 target/release/ei-type $(BINDIR)/ei-type

clean:
	rm -f ei-type
	rm -rf target
