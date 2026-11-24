//! A run of the same prepared bytes on wasmi and on the WebAssembly of the
//! host the tests run in, through `cartridge-web`'s `Browser`: what
//! `cartridge_test::on_every_engine` is where the tests are built to
//! WebAssembly, and run by wasm-bindgen-test-runner in node (V8). It needs
//! `cartridge-web` beside `cartridge-test`.

use cartridge_run::prepare;
use cartridge_test::{Run, on_wasmi, run};
use cartridge_web::Browser;

/// `file` run on wasmi and on the host's WebAssembly, which must agree to
/// the frame: the same frames, the same stop at the same update. Gives
/// what they agree on. A file `prepare` refuses runs on wasmi alone.
pub fn on_every_engine(file: &[u8], seed: u32, buttons: &[u32]) -> Run {
    let wasmi = on_wasmi(file, seed, buttons);
    let Ok(prepared) = prepare(file) else {
        return wasmi;
    };
    let host = run(Browser::cart(&prepared, seed), buttons);
    assert_eq!(host, wasmi, "the host's WebAssembly against wasmi");
    wasmi
}
