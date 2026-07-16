#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_json::{JsonDecodeOptions, LenientJsonDecoder};
use serde::Deserialize;

/// Minimal typed payload used to exercise typed decoder entry points.
#[derive(Deserialize)]
struct FuzzRecord;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    for decoder in [
        LenientJsonDecoder::default(),
        LenientJsonDecoder::new(JsonDecodeOptions::strict()),
    ] {
        let _ = decoder.decode::<FuzzRecord>(input);
        let _ = decoder.decode_object::<FuzzRecord>(input);
        let _ = decoder.decode_array::<FuzzRecord>(input);
        let _ = decoder.decode_value(input);
    }
});
