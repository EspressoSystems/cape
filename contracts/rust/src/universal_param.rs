#![deny(warnings)]
use jf_cap::testing_apis::universal_setup_for_test;
use lazy_static::lazy_static;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaChaRng;

const MAX_DEGREE_SUPPORTED: usize = 2u64.pow(17) as usize;

lazy_static! {
    pub static ref UNIVERSAL_PARAM: jf_cap::proof::UniversalParam =
        universal_setup_for_test(MAX_DEGREE_SUPPORTED, &mut ChaChaRng::from_seed([0u8; 32]))
            .unwrap();
}
