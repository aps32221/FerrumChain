//! WASM runtime build script (whitepaper §03/§04: WASM runtime executed by
//! every validator). Uses `substrate-wasm-builder` to compile this crate to
//! `wasm32-unknown-unknown` and embed it as the runtime blob.

fn main() {
    #[cfg(feature = "std")]
    {
        substrate_wasm_builder::WasmBuilder::build_using_defaults();
    }
}
