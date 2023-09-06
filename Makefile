PROJECT_NAME = lab_1
WIN_TARGET = x86_64-pc-windows-gnu
build:
	cargo build --target ${WIN_TARGET}
run: build
	wine target/${WIN_TARGET}/debug/${PROJECT_NAME}.exe