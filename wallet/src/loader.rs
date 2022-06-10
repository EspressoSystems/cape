// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Tools for creating, loading, and verifying CAPE wallets.

use crate::CapeWalletError;
use cap_rust_sandbox::{ledger::CapeLedger, model::Erc20Code};
use eqs::errors::EQSNetError;
use ethers::prelude::Address;
use net::client::{parse_error_body, response_body};
use seahorse::{
    hd::KeyTree,
    loader::{Loader, LoaderMetadata, WalletLoader},
    reader::Reader,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use surf::Url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CapeMetadata {
    pub load: LoaderMetadata,
    pub contract: Erc20Code,
}

pub struct CapeLoader {
    inner: Loader,
    contract: Erc20Code,
}

impl CapeLoader {
    pub fn new(dir: PathBuf, input: Reader, contract: Erc20Code) -> Self {
        Self {
            inner: Loader::new(dir, input),
            contract,
        }
    }

    pub fn from_literal(
        mnemonic: Option<String>,
        password: String,
        dir: PathBuf,
        contract: Erc20Code,
    ) -> Self {
        Self {
            inner: Loader::from_literal(mnemonic, password, dir),
            contract,
        }
    }

    pub fn recovery(mnemonic: String, password: String, dir: PathBuf, contract: Erc20Code) -> Self {
        Self {
            inner: Loader::recovery(mnemonic, password, dir),
            contract,
        }
    }

    pub async fn latest_contract(eqs: Url) -> Result<Erc20Code, CapeWalletError> {
        let eqs: surf::Client = surf::Config::default()
            .set_base_url(eqs)
            .try_into()
            .expect("Failed to configure EQS client");
        let eqs = eqs.with(parse_error_body::<EQSNetError>);
        let mut res = eqs
            .get("get_cape_contract_address")
            .send()
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("EQS error: {}", err),
            })?;
        let address: Address =
            response_body(&mut res)
                .await
                .map_err(|err| CapeWalletError::Failed {
                    msg: format!("Error parsing EQS response: {}", err),
                })?;
        Ok(address.into())
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}

impl WalletLoader<CapeLedger> for CapeLoader {
    type Meta = CapeMetadata;

    fn location(&self) -> PathBuf {
        WalletLoader::<CapeLedger>::location(&self.inner)
    }

    fn create(&mut self) -> Result<(CapeMetadata, KeyTree), CapeWalletError> {
        let (load, key) = self.inner.create()?;
        Ok((
            CapeMetadata {
                load,
                contract: self.contract.clone(),
            },
            key,
        ))
    }

    fn load(&mut self, meta: &mut CapeMetadata) -> Result<KeyTree, CapeWalletError> {
        if meta.contract != self.contract {
            return Err(CapeWalletError::Failed {
                msg: format!("keystore was created for CAPE contract at {}, but the current CAPE contract is {}",
                    meta.contract, self.contract)
            });
        }

        self.inner.load(&mut meta.load)
    }
}
