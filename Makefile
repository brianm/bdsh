VERSION := $(shell grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
HOST    := $(shell rustc -vV | grep host | cut -d' ' -f2)
TARGET  ?= $(HOST)

DIST_DIR     := dist/out
TARBALL_NAME := bdsh-$(VERSION)-$(TARGET)
TARBALL      := $(DIST_DIR)/$(TARBALL_NAME).tar.gz

# Binary location depends on whether we're cross-compiling
ifeq ($(TARGET),$(HOST))
  RELEASE_DIR := target/release
else
  RELEASE_DIR := target/$(TARGET)/release
endif

# Development
.PHONY: build test release clean

build:
	cargo build

test:
	cargo test

release:
ifeq ($(TARGET),$(HOST))
	cargo build --release
else
	cargo build --release --target $(TARGET)
endif

clean:
	cargo clean
	rm -rf $(DIST_DIR)

# Create release tarball for TARGET
# Usage: make dist TARGET=x86_64-unknown-linux-gnu
#    or: make dist  (native target)
.PHONY: dist

dist: release
	@mkdir -p $(DIST_DIR)/$(TARBALL_NAME)
	cp $(RELEASE_DIR)/bdsh $(DIST_DIR)/$(TARBALL_NAME)/
	cp $$(find target -name 'bdsh.1' -path '*/build/*/out/*' | head -1) $(DIST_DIR)/$(TARBALL_NAME)/
	cp LICENSE $(DIST_DIR)/$(TARBALL_NAME)/
	cd $(DIST_DIR) && tar -czvf $(TARBALL_NAME).tar.gz $(TARBALL_NAME)
	rm -rf $(DIST_DIR)/$(TARBALL_NAME)
	@echo "Created $(TARBALL)"

# Generate Homebrew formula
# Requires: all 4 platform tarballs in dist/out/
.PHONY: formula

formula:
	@VERSION=$(VERSION) DIST_DIR=$(DIST_DIR) ./dist/scripts/generate-formula.sh > $(DIST_DIR)/bdsh.rb
	@echo "Created $(DIST_DIR)/bdsh.rb"

# Generate AUR PKGBUILD
# Requires: source tarball URL to exist (uses GitHub release URL)
.PHONY: pkgbuild

pkgbuild:
	@VERSION=$(VERSION) ./dist/scripts/generate-pkgbuild.sh > $(DIST_DIR)/PKGBUILD
	@echo "Created $(DIST_DIR)/PKGBUILD"

# Print version (useful for scripts)
.PHONY: version

version:
	@echo $(VERSION)
