.PHONY: build clean test

build:
	cargo build --release -p hulk-driver
	cp target/release/hulkc ./hulk

clean:
	cargo clean
	rm -f ./hulk ./output

test: build
	bash tests/hulk/run_tests.sh . tests/hulk
