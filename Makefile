all: run

pull:
	git pull --recurse-submodules
	git submodule sync --recursive
	git submodule update --recursive --init

test:
	cargo test

build:
	cargo build

run:
	cargo run

clean:
	cargo clean