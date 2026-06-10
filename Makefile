.PHONY: run run-lan

run:
	cargo run --release -- $(ARGS)

run-lan:
	cargo run --release -- --lan -v $(ARGS)

tg://socks?server=10.2.1.77&port=1080
 