#![deny(warnings)]
use lazy_static::lazy_static;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaChaRng;
use zerok_lib::cape_state::CAPE_MERKLE_HEIGHT;

pub use universal_param::get as get_universal_param;
pub use universal_param::set as set_universal_param;

lazy_static! {
    pub static ref UNIVERSAL_PARAM: jf_aap::proof::UniversalParam =
        get_universal_param(&mut ChaChaRng::from_seed([0x8au8; 32]), CAPE_MERKLE_HEIGHT);
}
