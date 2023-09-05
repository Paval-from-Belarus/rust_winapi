PROJECT_NAME = lab_1
WIN_TARGET = x86_64-pc-windows-gnu
win32:
	cargo build --target ${WIN_TARGET}
build:
	cargo build
run: win32
	wine target/${WIN_TARGET}/debug/${PROJECT_NAME}.exe