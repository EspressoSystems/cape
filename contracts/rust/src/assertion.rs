//! naive assertion matcher for `ContractCall.call()` and `ContractCall.send()` tx.

use ethers::{abi::Detokenize, prelude::*};
use std::fmt::Debug;

pub(crate) trait Matcher {
    fn should_revert(self) -> bool;
    fn should_revert_with_message(self, message: &str) -> bool;
}

impl<D, M> Matcher for Result<D, ContractError<M>>
where
    D: Detokenize + Debug,
    M: Middleware,
{
    fn should_revert(self) -> bool {
        if self.is_err() {
            let e = format!("{}", self.unwrap_err());
            return e.contains("reverted");
        }
        false
    }

    fn should_revert_with_message(self, message: &str) -> bool {
        if self.is_err() {
            let e = format!("{}", self.unwrap_err());
            return e.contains("reverted") && e.contains(message);
        }
        false
    }
}
