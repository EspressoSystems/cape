use async_std::sync::Arc;
use async_trait::async_trait;
use cap_rust_sandbox::{deploy::EthMiddleware, ledger::*, model::*};
use jf_cap::{
    keys::UserAddress,
    structs::{AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordOpening},
};
use seahorse::{
    txn_builder::{TransactionInfo, TransactionReceipt},
    Wallet, WalletBackend, WalletError,
};

pub type CapeWalletError = WalletError<CapeLedger>;

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

    fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError>;
}

pub type CapeWallet<'a, Backend> = Wallet<'a, Backend, CapeLedger>;

#[async_trait]
pub trait CapeWalletExt<'a, Backend: CapeWalletBackend<'a> + Sync + 'a> {
    async fn sponsor(
        &mut self,
        erc20_code: Erc20Code,
        sponsor_addr: EthereumAddr,
        cap_asset_policy: AssetPolicy,
    ) -> Result<AssetDefinition, CapeWalletError>;

    // We may return a `WrapReceipt`, i.e., a record commitment to track wraps, once it's defined.
    //
    // It would be better to replace the `AssetDefinition` parameter with `AssetCode` to be
    // consistent with other transactions, but currently there is no way to inform the wallet of
    // the existence of an asset (so that it can convert a code to a definition) without owning it.
    async fn wrap(
        &mut self,
        src_addr: EthereumAddr,
        // We take as input the target asset, not the source ERC20 code, because there may be more
        // than one CAP asset for a given ERC20 token. We need the user to disambiguate (probably
        // using a list of approved (CAP, ERC20) pairs provided by the query service).
        cap_asset: AssetDefinition,
        dst_addr: UserAddress,
        amount: u64,
    ) -> Result<(), CapeWalletError>;

    /// For now, the amount to burn should be the same as a wrapped record.
    async fn burn(
        &mut self,
        account: &UserAddress,
        dst_addr: EthereumAddr,
        cap_asset: &AssetCode,
        amount: u64,
        fee: u64,
    ) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError>;

    async fn approved_assets(&self) -> Vec<(AssetDefinition, Erc20Code)>;
    async fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError>;
    async fn eth_address(&self) -> Result<EthereumAddr, CapeWalletError>;
}

#[async_trait]
impl<'a, Backend: CapeWalletBackend<'a> + Sync + 'a> CapeWalletExt<'a, Backend>
    for CapeWallet<'a, Backend>
{
    async fn sponsor(
        &mut self,
        erc20_code: Erc20Code,
        sponsor_addr: EthereumAddr,
        cap_asset_policy: AssetPolicy,
    ) -> Result<AssetDefinition, CapeWalletError> {
        let mut state = self.lock().await;

        let description = erc20_asset_description(&erc20_code, &sponsor_addr);

        //todo Include CAPE-specific domain separator in AssetCode derivation, once Jellyfish adds
        // support for domain separators.
        let code = AssetCode::new_foreign(&description);
        let asset = AssetDefinition::new(code, cap_asset_policy)
            .map_err(|source| CapeWalletError::CryptoError { source })?;

        state
            .backend_mut()
            .register_erc20_asset(&asset, erc20_code, sponsor_addr)
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
        let xfr_info = self
            // The owner public key of the new record opening is ignored when processing a burn. We
            // need to put some address in the receiver field though, so just use the one we have
            // handy.
            .build_transfer(
                account,
                cap_asset,
                &[(account.clone(), amount)],
                fee,
                bound_data,
                Some((2, 2)),
            )
            .await?;

        // Only generate memos for the fee change output.
        assert!(xfr_info.fee_output.is_some());
        let (memos, sig) = self
            .generate_memos(vec![xfr_info.fee_output.unwrap()], &xfr_info.sig_keypair)
            .await?;

        let mut txn_info = TransactionInfo {
            account: xfr_info.owner_address,
            memos,
            sig,
            freeze_outputs: vec![],
            history: Some(xfr_info.history),
            uid: None,
            inputs: xfr_info.inputs,
            outputs: xfr_info.outputs,
        };
        assert_eq!(xfr_info.note.inputs_nullifiers.len(), 2);
        assert_eq!(xfr_info.note.output_commitments.len(), 2);
        if let Some(history) = &mut txn_info.history {
            history.kind = CapeTransactionKind::Burn;
        }

        let txn = CapeTransition::Transaction(CapeModelTxn::Burn {
            xfr: Box::new(xfr_info.note),
            ro: Box::new(txn_info.outputs[0].clone()),
        });
        self.submit(txn, txn_info).await
    }

    async fn approved_assets(&self) -> Vec<(AssetDefinition, Erc20Code)> {
        unimplemented!()
    }

    async fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError> {
        self.lock().await.backend().eth_client()
    }

    async fn eth_address(&self) -> Result<EthereumAddr, CapeWalletError> {
        Ok(self.eth_client().await?.address().into())
    }
}
