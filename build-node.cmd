@echo off
REM ===================================================================
REM  Ferrum - one-command Windows node build (see BUILD.md section 2.4).
REM  Sets the full toolchain environment and builds ferrum-node.
REM  Edit the literal paths below if your install differs.
REM
REM  Usage:   build-node.cmd          (release build of the node)
REM           build-node.cmd -p ferrum-runtime   (pass extra cargo args)
REM
REM  Requires (one-time): protoc + LLVM/clang at the paths below,
REM  rustup toolchains 1.95.0 (host) and 1.81.0 (runtime WASM), and the
REM  sc-network cache patch (BUILD.md 2.4 - non-reproducible!).
REM ===================================================================

set "PROTOC=C:\Users\Cat\tools\protoc\bin\protoc.exe"
set "CC=C:\Users\Cat\tools\LLVM\bin\clang-cl.exe"
set "CXX=C:\Users\Cat\tools\LLVM\bin\clang-cl.exe"
set "CXXFLAGS=/EHsc"
set "LIBCLANG_PATH=C:\Users\Cat\tools\LLVM\bin"
REM Forward slashes - bindgen arg parser eats backslashes as escapes.
set "BINDGEN_EXTRA_CLANG_ARGS=-IC:/Users/Cat/tools/LLVM/lib/clang/22/include"
set "WASM_BUILD_TOOLCHAIN=1.81.0-x86_64-pc-windows-msvc"

if "%~1"=="" (
    rustup run 1.95.0 cargo build --release --locked -p ferrum-node
) else (
    rustup run 1.95.0 cargo build --release --locked %*
)
