ARGS ?=
SUDO := sudo -E
KUBERNIX := $(SUDO) target/release/kubernix $(ARGS)

define nix-shell
	nix-shell -j$(shell nproc) nix/build.nix $(1)
endef

define nix-shell-pure
	$(call nix-shell,--keep SSH_AUTH_SOCK --pure $(1))
endef

define nix-shell-run
	$(call nix-shell,--run "$(1)")
endef

define nix-shell-pure-run
	$(call nix-shell-pure,--run "$(1)")
endef

all: build

.PHONY: build
build:
	$(call nix-shell-pure-run,cargo build)

.PHONY: build-release
build-release:
	$(call nix-shell-pure-run,cargo build --release)

.PHONY: coverage
coverage:
	$(call nix-shell-pure-run,cargo kcov)

.PHONY: nix-shell
nix-shell:
	$(call nix-shell-pure)

.PHONY: docs
docs:
	$(call nix-shell-pure-run,cargo doc --no-deps)

.PHONY: nixpkgs
nixpkgs:
	nix-shell -p nix-prefetch-git --run "nix-prefetch-git --no-deepClone \
		https://github.com/nixos/nixpkgs > nix/nixpkgs.json"

.PHONY: shell
shell: build-release
	$(KUBERNIX) shell

.PHONY: test-integration
test-integration: build-release
	$(SUDO) test/integration

.PHONY: test-unit
test-unit:
	$(call nix-shell-pure-run,cargo test)

.PHONY: run
run: build-release
	$(KUBERNIX)

.PHONY: lint-clippy
lint-clippy:
	$(call nix-shell-pure-run,cargo clippy --all -- -D warnings)

.PHONY: lint-rustfmt
lint-rustfmt:
	$(call nix-shell-pure-run,cargo fmt && git diff --exit-code)
