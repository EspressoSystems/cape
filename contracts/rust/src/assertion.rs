//! naive assertion matcher for `ContractCall.call()` and `ContractCall.send()` tx.
use ethers::{abi::Detokenize, prelude::*};
use std::fmt::Debug;

pub(crate) trait Matcher {
    fn should_revert(self);
    fn should_revert_with_message(self, message: &str);
}

fn check_contains(string: &str, sub_string: &str) {
    if !string.contains(sub_string) {
        panic!("Sub-string \"{}\" not found in \"{}\"", sub_string, string);
    }
}

impl<D, M> Matcher for Result<D, ContractError<M>>
where
    D: Detokenize + Debug,
    M: Middleware,
{
    fn should_revert(self) {
        if self.is_ok() {
            panic!("Not reverted");
        }
        check_contains(&self.unwrap_err().to_string(), "reverted");
    }

    fn should_revert_with_message(self, message: &str) {
        if self.is_ok() {
            panic!("Not reverted");
        }

        let error = self.unwrap_err().to_string();
        check_contains(&error, "reverted");
        check_contains(&error, message);
    }
}
