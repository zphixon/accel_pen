#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(node) = gbx_rs::Node::read_from(&data) {
        let _ = node.parse();
    }
});
