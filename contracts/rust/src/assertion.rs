//! naive assertion matcher for `ContractCall.call()` and `ContractCall.send()` tx.
use ethers::{abi::Detokenize, prelude::*};
use std::fmt::Debug;

pub trait Matcher {
    fn should_not_revert(self);
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
    fn should_not_revert(self) {
        if self.is_err() {
            panic!(
                "Tx should not revert but it reverted with \"{}\"",
                self.unwrap_err()
            );
        }
    }

    fn should_revert(self) {
        if self.is_ok() {
            panic!("Tx should revert but it did not revert");
        }
        check_contains(&self.unwrap_err().to_string(), "reverted");
    }

    fn should_revert_with_message(self, message: &str) {
        if self.is_ok() {
            panic!(
                "Tx should revert with \"{}\" but it did not revert",
                message
            );
        }

        let error = self.unwrap_err().to_string();
        check_contains(&error, "reverted");
        check_contains(&error, message);
    }
}

pub trait EnsureMined {
    fn ensure_mined(self) -> Self;
}

impl EnsureMined for Option<TransactionReceipt> {
    fn ensure_mined(self) -> Self {
        if self.as_ref().unwrap().status.unwrap() != U64::from(1) {
            panic!("Transaction not mined");
        }
        self
    }
}
