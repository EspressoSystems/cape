// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Instantiation of [seahorse::Wallet] for CAPE.

use async_std::{fs, sync::Arc};
use async_trait::async_trait;
use cap_rust_sandbox::{deploy::EthMiddleware, ledger::*, model::*};
use jf_cap::{
    keys::UserAddress,
    structs::{AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordOpening},
    VerKey,
};
use seahorse::{txn_builder::TransactionReceipt, AssetInfo, Wallet, WalletBackend, WalletError};
use std::path::Path;

pub type CapeWalletError = WalletError<CapeLedger>;

/// Extension of the [WalletBackend] trait with CAPE-specific functionality.
#[async_trait]
pub trait CapeWalletBackend<'a>: WalletBackend<'a, CapeLedger> {
    /// Update the global ERC20 asset registry with a new (ERC20, CAPE asset) pair.
    ///
    /// There may only be one ERC20 token registered for each CAPE asset. If `asset` has already
    /// been used to register an ERC20 token, this function must fail.
    async fn register_erc20_asset(
        &mut self,
        asset: &AssetDefinition,
        erc20_code: Erc20Code,
        sponsor: EthereumAddr,
    ) -> Result<(), CapeWalletError>;

    /// Get the ERC20 code which is associated with a CAPE asset.
    async fn get_wrapped_erc20_code(
        &self,
        asset: &AssetDefinition,
    ) -> Result<Erc20Code, CapeWalletError>;

    /// Wrap some amount of an ERC20 token in a CAPE asset.
    ///
    /// The amount to wrap is determined by the `amount` field of `ro`. The CAPE asset type
    /// (`ro.asset_def`) must be registered as a CAPE wrapper for `erc20_code` (see
    /// `register_erc20_asset`). The linked Ethereum wallet with `src_addr` must own at least
    /// `ro.amount` of `erc20_code`.
    ///
    /// The new CAPE balance will not be reflected until the wrap is finalized, the next time a
    /// block is validated by the contract, but once this function succeeds the ERC20 balance will
    /// be deducted from the linked Ethereum account and the CAPE assets will be guaranteed at the
    /// next block.
    async fn wrap_erc20(
        &mut self,
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: RecordOpening,
    ) -> Result<(), CapeWalletError>;

    /// Get the underlying Ethereum connection.
    fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError>;

    /// Get the official verification key used to verify asset libraries.
    fn asset_verifier(&self) -> VerKey;
}

pub type CapeWallet<'a, Backend> = Wallet<'a, Backend, CapeLedger>;

/// Extension methods for CAPE wallets.
///
/// This trait adds to [Wallet] some methods that implement CAPE-specific functionality. It is
/// automatically implemented for any instantiation of [Wallet] with a backend that implements
/// [CapeWalletBackend].
#[async_trait]
pub trait CapeWalletExt<'a, Backend: CapeWalletBackend<'a> + Sync + 'a> {
    /// Sponsor the creation of a new wrapped ERC-20 CAPE asset.
    async fn sponsor(
        &mut self,
        symbol: String,
        erc20_code: Erc20Code,
        sponsor_addr: EthereumAddr,
        cap_asset_policy: AssetPolicy,
    ) -> Result<AssetDefinition, CapeWalletError>;

    /// Wrap some ERC-20 tokens into a CAPE asset.
    ///
    /// This function will withdraw `amount` tokens from the account with address `src_addr` of the
    /// ERC-20 token corresponding to the CAPE asset `cap_asset`. `src_addr` must be one of the
    /// addresses of the ETH wallet backing this CAPE wallet. [CapeWalletExt::eth_address] will
    /// return a valid address to be used here.
    async fn wrap(
        &mut self,
        src_addr: EthereumAddr,
        // We take as input the target asset, not the source ERC20 code, because there may be more
        // than one CAP asset for a given ERC20 token. We need the user to disambiguate (probably
        // using a list of approved (CAP, ERC20) pairs provided by the query service).
        //
        // It would be better to replace the `AssetDefinition` parameter with `AssetCode` to be
        // consistent with other transactions, but currently there is no way to inform the wallet of
        // the existence of an asset (so that it can convert a code to a definition) without owning
        // it.
        cap_asset: AssetDefinition,
        dst_addr: UserAddress,
        amount: u64,
        // We may return a `WrapReceipt`, i.e., a record commitment to track wraps, once it's defined.
    ) -> Result<(), CapeWalletError>;

    /// Burn some wrapped tokens, unlocking the corresponding ERC-20 tokens into the account
    /// `dst_addr`.
    ///
    /// `cap_asset` must be a wrapped asset type.
    ///
    /// The amount to burn must match exactly the amount of a wrapped record owned by `account`, as
    /// burn transactions with change are not supported. This restriction will be lifted in the
    /// future.
    async fn burn(
        &mut self,
        account: &UserAddress,
        dst_addr: EthereumAddr,
        cap_asset: &AssetCode,
        amount: u64,
        fee: u64,
    ) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError>;

    /// Get the ERC-20 asset code that this asset wraps, if this is a wrapped asset.
    async fn wrapped_asset(&self, asset: AssetCode) -> Option<Erc20Code>;

    /// Determine if an asset is a wrapped ERC-20 asset (as opposed to a domestic CAPE asset).
    async fn is_wrapped_asset(&self, asset: AssetCode) -> bool;

    /// Get the underlying Ethereum connection.
    async fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError>;

    /// Get an address owned by the underlying Ethereum wallet.
    async fn eth_address(&self) -> Result<EthereumAddr, CapeWalletError>;

    /// Import an asset library signed by the official CAPE asset signing key.
    async fn verify_cape_assets(
        &mut self,
        library: &Path,
    ) -> Result<Vec<AssetInfo>, CapeWalletError>;
}

#[async_trait]
impl<'a, Backend: CapeWalletBackend<'a> + Sync + 'a> CapeWalletExt<'a, Backend>
    for CapeWallet<'a, Backend>
{
    async fn sponsor(
        &mut self,
        symbol: String,
        erc20_code: Erc20Code,
        sponsor_addr: EthereumAddr,
        cap_asset_policy: AssetPolicy,
    ) -> Result<AssetDefinition, CapeWalletError> {
        let mut state = self.lock().await;

        let description = erc20_asset_description(&erc20_code, &sponsor_addr);
        let code = AssetCode::new_foreign(&description);
        let asset = AssetDefinition::new(code, cap_asset_policy)
            .map_err(|source| CapeWalletError::CryptoError { source })?;

        state
            .backend_mut()
            .register_erc20_asset(&asset, erc20_code, sponsor_addr)
            .await?;
        drop(state);

        // Add the new asset to our asset library.
        self.import_asset(AssetInfo::from(asset.clone()).with_name(symbol))
            .await?;

        Ok(asset)
    }

    async fn wrap(
        &mut self,
        src_addr: EthereumAddr,
        cap_asset: AssetDefinition,
        dst_addr: UserAddress,
        amount: u64,
    ) -> Result<(), CapeWalletError> {
        let mut state = self.lock().await;

        let erc20_code = state.backend().get_wrapped_erc20_code(&cap_asset).await?;
        let pub_key = state.backend().get_public_key(&dst_addr).await?;

        let ro = RecordOpening::new(
            state.rng(),
            amount,
            cap_asset,
            pub_key,
            FreezeFlag::Unfrozen,
        );

        state
            .backend_mut()
            .wrap_erc20(erc20_code, src_addr, ro)
            .await
    }

    async fn burn(
        &mut self,
        account: &UserAddress,
        dst_addr: EthereumAddr,
        cap_asset: &AssetCode,
        amount: u64,
        fee: u64,
    ) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError> {
        // A burn note is just a transfer note with a special `proof_bound_data` field consisting of
        // the magic burn bytes followed by the destination address.
        let bound_data = CAPE_BURN_MAGIC_BYTES
            .as_bytes()
            .iter()
            .chain(dst_addr.as_bytes())
            .cloned()
            .collect::<Vec<_>>();
        let (note, mut info) = self
            // The owner public key of the new record opening is ignored when processing a burn. We
            // need to put some address in the receiver field though, so just use the one we have
            // handy.
            .build_transfer(
                Some(account),
                cap_asset,
                &[(account.clone(), amount, true)],
                fee,
                bound_data,
                Some((2, 2)),
            )
            .await?;

        assert_eq!(note.inputs_nullifiers.len(), 2);
        assert_eq!(note.output_commitments.len(), 2);
        if let Some(history) = &mut info.history {
            history.kind = CapeTransactionKind::Burn;
        }

        let txn = CapeTransition::Transaction(CapeModelTxn::Burn {
            xfr: Box::new(note),
            ro: Box::new(info.outputs[1].clone()),
        });
        self.submit(txn, info).await
    }

    async fn wrapped_asset(&self, asset: AssetCode) -> Option<Erc20Code> {
        let asset = self.asset(asset).await?;
        let state = self.lock().await;
        state
            .backend()
            .get_wrapped_erc20_code(&asset.definition)
            .await
            .ok()
    }

    async fn is_wrapped_asset(&self, asset: AssetCode) -> bool {
        self.wrapped_asset(asset).await.is_some()
    }

    async fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError> {
        self.lock().await.backend().eth_client()
    }

    async fn eth_address(&self) -> Result<EthereumAddr, CapeWalletError> {
        Ok(self.eth_client().await?.address().into())
    }

    async fn verify_cape_assets(
        &mut self,
        library: &Path,
    ) -> Result<Vec<AssetInfo>, CapeWalletError> {
        let bytes = fs::read(library)
            .await
            .map_err(|source| CapeWalletError::IoError { source })?;
        let library = bincode::deserialize(&bytes)?;
        let ver_key = self.lock().await.backend().asset_verifier();
        self.verify_assets(&ver_key, library).await
    }
}
