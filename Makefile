WASP_VERSION ?= '9cc5cbd1134744de4c805f5f685dd03599e07c6f'

build: wasp
	@cargo build --release
	@rm -rf ./release
	@mkdir -p ./release
	@cp ./target/release/wasp-cli ./release/wasp -f
	@strip ./release/wasp

wasp:
	@curl -Ln https://github.com/camshaft/wasp/archive/$(WASP_VERSION).tar.gz | tar xz
	@mv wasp-$(WASP_VERSION) wasp
