#![deny(warnings)]
use crate::{state::CapeTransaction, types as sol};
use anyhow::{anyhow, bail, Result};
use ark_serialize::*;
use ethers::prelude::Address;
use jf_aap::freeze::FreezeNote;
use jf_aap::keys::UserAddress;
use jf_aap::mint::MintNote;
use jf_aap::structs::{RecordCommitment, RecordOpening};
use jf_aap::transfer::TransferNote;
use jf_aap::TransactionNote;
use num_traits::{FromPrimitive, ToPrimitive};
use std::str::from_utf8;

pub const DOM_SEP_CAPE_BURN: &[u8] = b"TRICAPE burn";

/// Burning transaction structure for a single asset (with fee)
#[derive(Debug, PartialEq, Eq, Hash, Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct BurnNote {
    /// Burn is effectively a transfer, this is the txn note.
    pub transfer_note: TransferNote,
    /// Record opening of the burned output (2nd in the transfer).
    pub burned_ro: RecordOpening,
}

impl BurnNote {
    /// Construct a `BurnNote` using the underlying transfer note and the burned
    /// record opening (namely of the second output)
    pub fn generate(note: TransferNote, burned_ro: RecordOpening) -> Result<Self> {
        if note.output_commitments.len() < 2
            || note.output_commitments[1] != RecordCommitment::from(&burned_ro)
            || note.aux_info.extra_proof_bound_data.len() != 32
            || !Self::is_burn_note(&note)
        {
            bail!("Malformed Burned Note parameters");
        }
        Ok(Self {
            transfer_note: note,
            burned_ro,
        })
    }

    /// Retrieve the Ethereum recipient address
    pub fn withdraw_recipient(&self) -> Result<Address> {
        from_utf8(&self.transfer_note.aux_info.extra_proof_bound_data[DOM_SEP_CAPE_BURN.len()..])?
            .parse::<Address>()
            .map_err(|_| anyhow!("Invalid Ethereum address!"))
    }

    /// utility function to check if a `TransferNote` is a `BurnNote`
    pub fn is_burn_note(note: &TransferNote) -> bool {
        note.aux_info
            .extra_proof_bound_data
            .starts_with(DOM_SEP_CAPE_BURN)
    }
}

impl From<BurnNote> for sol::BurnNote {
    fn from(note: BurnNote) -> Self {
        Self {
            transfer_note: note.transfer_note.into(),
            record_opening: note.burned_ro.into(),
        }
    }
}

impl From<sol::BurnNote> for BurnNote {
    fn from(note_sol: sol::BurnNote) -> Self {
        Self {
            transfer_note: note_sol.transfer_note.into(),
            burned_ro: note_sol.record_opening.into(),
        }
    }
}

/// A cape block containing a batch of transaction notes.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct CapeBlock {
    /// miner (a.k.a fee collector)
    pub miner_addr: UserAddress,
    /// the ordering of txn within the block
    pub note_types: Vec<NoteType>,
    /// sorted transfer notes
    pub transfer_notes: Vec<TransferNote>,
    /// sorted mint notes
    pub mint_notes: Vec<MintNote>,
    /// sorted freeze notes
    pub freeze_notes: Vec<FreezeNote>,
    /// sorted burn notes
    pub burn_notes: Vec<BurnNote>,
}

impl CapeBlock {
    /// Generate a CapeBlock
    pub fn generate(
        notes: Vec<TransactionNote>,
        burned_ros: Vec<RecordOpening>,
        miner: UserAddress,
    ) -> Result<Self> {
        let mut transfer_notes = vec![];
        let mut mint_notes = vec![];
        let mut freeze_notes = vec![];
        let mut burn_notes = vec![];
        let mut note_types = vec![];
        for note in notes {
            match note {
                TransactionNote::Transfer(n) => {
                    if BurnNote::is_burn_note(&n) {
                        burn_notes.push(*n);
                        note_types.push(NoteType::Burn);
                    } else {
                        transfer_notes.push(*n);
                        note_types.push(NoteType::Transfer);
                    }
                }
                TransactionNote::Mint(n) => {
                    mint_notes.push(*n);
                    note_types.push(NoteType::Mint);
                }
                TransactionNote::Freeze(n) => {
                    freeze_notes.push(*n);
                    note_types.push(NoteType::Freeze);
                }
            }
        }

        if burn_notes.len() != burned_ros.len() {
            bail!("Mismatched number of burned openings");
        }
        let burn_notes: Vec<BurnNote> = burn_notes
            .iter()
            .zip(burned_ros.iter())
            .map(|(note, ro)| BurnNote::generate(note.clone(), ro.clone()).unwrap())
            .collect();

        Ok(Self {
            miner_addr: miner,
            note_types,
            transfer_notes,
            mint_notes,
            freeze_notes,
            burn_notes,
        })
    }

    pub fn from_cape_transactions(
        transactions: Vec<CapeTransaction>,
        miner: UserAddress,
    ) -> Result<Self> {
        let mut burned_ros = vec![];
        let mut notes = vec![];

        for tx in transactions {
            match tx {
                CapeTransaction::AAP(note) => notes.push(note),
                CapeTransaction::Burn { xfr, ro } => {
                    notes.push(TransactionNote::from(*xfr));
                    burned_ros.push(*ro);
                }
            }
        }
        Self::generate(notes, burned_ros, miner)
    }
}

impl From<CapeBlock> for sol::CapeBlock {
    fn from(blk: CapeBlock) -> Self {
        Self {
            miner_addr: blk.miner_addr.into(),
            note_types: blk
                .note_types
                .iter()
                .map(|t| t.to_u8().unwrap_or(0))
                .collect(),
            transfer_notes: blk
                .transfer_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            mint_notes: blk.mint_notes.iter().map(|n| n.clone().into()).collect(),
            freeze_notes: blk.freeze_notes.iter().map(|n| n.clone().into()).collect(),
            burn_notes: blk.burn_notes.iter().map(|n| n.clone().into()).collect(),
        }
    }
}

impl From<sol::CapeBlock> for CapeBlock {
    fn from(blk_sol: sol::CapeBlock) -> Self {
        Self {
            miner_addr: blk_sol.miner_addr.into(),
            note_types: blk_sol
                .note_types
                .iter()
                .map(|t| NoteType::from_u8(*t).unwrap_or(NoteType::Transfer))
                .collect(),
            transfer_notes: blk_sol
                .transfer_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            mint_notes: blk_sol
                .mint_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            freeze_notes: blk_sol
                .freeze_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            burn_notes: blk_sol
                .burn_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
        }
    }
}

/// Note type available in CAPE.
#[derive(FromPrimitive, ToPrimitive, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteType {
    Transfer,
    Mint,
    Freeze,
    Burn,
}

impl From<TransactionNote> for NoteType {
    fn from(note: TransactionNote) -> Self {
        match note {
            TransactionNote::Transfer(n) => {
                if BurnNote::is_burn_note(&n) {
                    Self::Burn
                } else {
                    Self::Transfer
                }
            }
            TransactionNote::Mint(_) => Self::Mint,
            TransactionNote::Freeze(_) => Self::Freeze,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CAPEConstructorArgs {
    height: u8,
    n_roots: u64,
    verifier_addr: Address,
}

#[allow(dead_code)]
impl CAPEConstructorArgs {
    pub fn new(height: u8, n_roots: u64, verifier_addr: Address) -> Self {
        Self {
            height,
            n_roots,
            verifier_addr,
        }
    }
}

impl From<CAPEConstructorArgs> for (u8, u64, Address) {
    fn from(args: CAPEConstructorArgs) -> (u8, u64, Address) {
        (args.height, args.n_roots, args.verifier_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertion::Matcher;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::ledger::CapeLedger;
    use crate::state::{erc20_asset_description, Erc20Code, EthereumAddr};
    use crate::types::{
        AssetCodeSol, GenericInto, InternalAssetCodeSol, MerkleRootSol, NullifierSol,
        RecordCommitmentSol, TestCAPE, TestCapeTypes,
    };
    use anyhow::Result;
    use ethers::prelude::{
        k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet, U256,
    };
    use jf_aap::keys::{UserKeyPair, UserPubKey};
    use jf_aap::structs::{AssetCode, AssetCodeSeed, InternalAssetCode, RecordOpening};
    use jf_aap::utils::TxnsParams;
    use rand::Rng;
    use reef::Ledger;
    use std::path::Path;

    async fn deploy_cape_test() -> TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
        let client = get_funded_deployer().await.unwrap();
        // deploy the PlonkVerifier
        let verifier = deploy(
            client.clone(),
            Path::new("../abi/contracts/verifier/PlonkVerifier.sol/PlonkVerifier"),
            (),
        )
        .await
        .unwrap();

        // deploy TestCAPE.sol
        let contract = deploy(
            client.clone(),
            Path::new("../abi/contracts/mocks/TestCAPE.sol/TestCAPE"),
            CAPEConstructorArgs::new(24, 10, verifier.address())
                .generic_into::<(u8, u64, Address)>(),
        )
        .await
        .unwrap();
        TestCAPE::new(contract.address(), client)
    }

    #[tokio::test]
    async fn test_batch_verify_validity_proof() -> Result<()> {
        let rng = &mut ark_std::test_rng();

        // Create a block with 3 transfer, 1 mint, 2 freeze
        let params = TxnsParams::generate_txns(rng, 3, 1, 2, CapeLedger::merkle_height());
        let miner = UserKeyPair::generate(rng);

        // simulate initial contract state to contain those record to be consumed
        let contract = deploy_cape_test().await;
        for txn in params.txns.iter() {
            contract
                .add_root(txn.merkle_root().generic_into::<MerkleRootSol>().0)
                .send()
                .await?
                .await?;
        }

        // submit the block during which validity proofs would be verified in batch
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;
        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_empty_block_to_cape_contract() -> Result<()> {
        // Create an empty block transactions
        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 0, 0, 0, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        // Submitting an empty block does not yield a reject from the contract
        let contract = deploy_cape_test().await;
        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;

        // The height is incremented anyways.
        assert_eq!(contract.height().call().await?, 1u64);

        Ok(())
    }

    #[tokio::test]
    async fn test_submit_block_to_cape_contract() -> Result<()> {
        // Create three transactions
        let rng = &mut ark_std::test_rng();
        let num_transfer_txn = 1;
        let num_mint_txn = 1;
        let num_freeze_txn = 1;
        let params = TxnsParams::generate_txns(
            rng,
            num_transfer_txn,
            num_mint_txn,
            num_freeze_txn,
            CapeLedger::merkle_height(),
        );
        let miner = UserPubKey::default();

        let nf = params.txns[0].nullifiers()[0];
        let root = params.txns[0].merkle_root();

        // temporarily no burn txn yet.
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        // Check that some nullifier is not yet inserted
        let contract = deploy_cape_test().await;
        assert!(
            !contract
                .nullifiers(nf.generic_into::<NullifierSol>().0)
                .call()
                .await?
        );

        // TODO should not require to manually submit the root here
        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        // Submit to the contract
        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;

        // Check that now the nullifier has been inserted
        assert!(
            contract
                .nullifiers(nf.generic_into::<NullifierSol>().0)
                .call()
                .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_block_height() -> Result<()> {
        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let root = params.txns[0].merkle_root();
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        // TODO should not require to manually submit the root here
        let contract = deploy_cape_test().await;
        assert_eq!(contract.height().call().await?, 0u64);

        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;
        assert_eq!(contract.height().call().await?, 1u64);

        Ok(())
    }

    #[tokio::test]
    async fn test_event_block_committed() -> Result<()> {
        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let root = params.txns[0].merkle_root();
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        let contract = deploy_cape_test().await;
        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;

        let logs = contract
            .block_committed_filter()
            .from_block(0u64)
            .query()
            .await?;
        assert_eq!(logs[0].height, 1);
        assert_eq!(logs[0].included_notes, vec![true]);
        Ok(())
    }

    // TODO add a test to check if includedNotes is computed correctly

    #[test]
    fn test_note_types() {
        // TODO
        // let rng = ark_std::test_rng();
        // assert_eq!(get_note_type(TransferNote::rand_for_test(&rng)), 0u8);
        // assert_eq!(get_note_type(FreezeNote::rand_for_test(&rng)), 1u8);
        // assert_eq!(get_note_type(MintNote::rand_for_test(&rng)), 2u8);
        // assert_eq!(get_note_type(create_test_burn_note(&rng)), 3u8);
    }

    #[tokio::test]
    async fn test_contains_burn_prefix() {
        let contract = deploy_cape_test().await;

        let dom_sep_str = std::str::from_utf8(DOM_SEP_CAPE_BURN).unwrap();
        for (input, expected) in [
            ("", false),
            ("x", false),
            ("TRICAPE bur", false),
            ("more data but but still not a burn", false),
            (dom_sep_str, true),
            (&(dom_sep_str.to_owned() + "more stuff"), true),
        ] {
            assert_eq!(
                contract
                    .contains_burn_prefix(input.as_bytes().to_vec().into())
                    .call()
                    .await
                    .unwrap(),
                expected
            )
        }
    }

    #[tokio::test]
    async fn test_contains_burn_destination() {
        let contract = deploy_cape_test().await;

        for (input, expected) in [
            (
                // ok, zero address from byte 12 to 32
                "ffffffffffffffffffffffff0000000000000000000000000000000000000000",
                true,
            ),
            (
                // ok, with more data
                "ffffffffffffffffffffffff0000000000000000000000000000000000000000ff",
                true,
            ),
            (
                // wrong address
                "ffffffffffffffffffffffff0000000000000000000000000000000000000001",
                false,
            ),
            (
                // address too short
                "ffffffffffffffffffffffff00000000000000000000000000000000000000",
                false,
            ),
        ] {
            assert_eq!(
                contract
                    .contains_burn_destination(hex::decode(input).unwrap().into())
                    .call()
                    .await
                    .unwrap(),
                expected
            )
        }
    }

    #[tokio::test]
    async fn test_contains_burn_record() {
        let contract = deploy_cape_test().await;

        assert!(!contract
            .contains_burn_record(sol::BurnNote::default())
            .call()
            .await
            .unwrap());

        // TODO test with a valid note
        // let mut rng = ark_std::test_rng();
        // let note = TransferNote::...
        // let burned_ro = RecordOpening::rand_for_test(&mut rng);
        // let burn_note = BurnNote::generate(note, burned_ro);
        // assert!(contract.contains_burn_record(burn_note).call().await.unwrap());
    }

    #[tokio::test]
    async fn test_check_burn_bad_prefix() {
        let contract = deploy_cape_test().await;
        let mut note = sol::BurnNote::default();
        let extra = [
            hex::decode("ffffffffffffffffffffffff").unwrap(),
            hex::decode(b"0000000000000000000000000000000000000000").unwrap(),
        ]
        .concat();
        note.transfer_note.aux_info.extra_proof_bound_data = extra.into();

        let call = contract.check_burn(note).call().await;
        call.should_revert_with_message("Bad burn tag");
    }

    #[tokio::test]
    async fn test_check_burn_bad_destination() {
        let contract = deploy_cape_test().await;
        let mut note = sol::BurnNote::default();
        let extra = [
            DOM_SEP_CAPE_BURN.to_vec(),
            hex::decode("000000000000000000000000000000000000000f").unwrap(),
        ]
        .concat();
        note.transfer_note.aux_info.extra_proof_bound_data = extra.into();

        let call = contract.check_burn(note).call().await;
        call.should_revert_with_message("Bad burn destination");
    }

    #[tokio::test]
    async fn test_check_burn_bad_record_commitment() {
        let contract = deploy_cape_test().await;
        let mut note = sol::BurnNote::default();
        let extra = [
            DOM_SEP_CAPE_BURN.to_vec(),
            hex::decode("0000000000000000000000000000000000000000").unwrap(),
        ]
        .concat();
        note.transfer_note.aux_info.extra_proof_bound_data = extra.into();

        note.transfer_note.output_commitments.push(U256::from(1));
        note.transfer_note.output_commitments.push(U256::from(2));

        let call = contract.check_burn(note).call().await;
        call.should_revert_with_message("Bad record commitment");
    }

    // TODO Add test for check_burn that passes

    // TODO integration test to check if check_burn is hooked up correctly in
    // main block validaton loop.

    #[tokio::test]
    async fn test_check_transfer_expired_note_removed() -> Result<()> {
        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let tx = params.txns[0].clone();
        let root = tx.merkle_root();
        let nf = tx.nullifiers()[0];
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;
        let valid_until = match tx {
            TransactionNote::Transfer(note) => note.aux_info.valid_until,
            TransactionNote::Mint(_) => todo!(),
            TransactionNote::Freeze(_) => todo!(),
        };

        // Set the height to expire note
        let contract = deploy_cape_test().await;
        contract.set_height(valid_until + 1).send().await?.await?;

        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;

        // Verify nullifier *not* spent
        let client = get_funded_deployer().await?;
        let contract = TestCAPE::new(contract.address(), client);
        assert!(
            !contract
                .nullifiers(nf.generic_into::<NullifierSol>().0)
                .call()
                .await?
        );

        // Check that the height increased by one
        assert_eq!(contract.height().call().await?, valid_until + 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_check_transfer_note_with_burn_prefix_rejected() {
        let contract = deploy_cape_test().await;
        let mut note = sol::TransferNote::default();
        let extra = [
            DOM_SEP_CAPE_BURN.to_vec(),
            hex::decode("0000000000000000000000000000000000000000").unwrap(),
        ]
        .concat();
        note.aux_info.extra_proof_bound_data = extra.into();

        let call = contract.check_transfer(note).call().await;
        call.should_revert_with_message("Burn prefix in transfer note");
    }

    #[tokio::test]
    async fn test_check_transfer_without_burn_prefix_accepted() {
        let contract = deploy_cape_test().await;
        let note = sol::TransferNote::default();
        assert!(contract.check_transfer(note).call().await.is_ok());
    }

    // TODO integration test to check if check_transfer is hooked up correctly in
    // main block validaton loop.

    #[tokio::test]
    async fn test_derive_record_commitment() {
        let contract = deploy_cape_test().await;
        let mut rng = ark_std::test_rng();

        for _run in 0..10 {
            let ro = RecordOpening::rand_for_test(&mut rng);
            let rc = RecordCommitment::from(&ro);

            let rc_u256 = contract
                .derive_record_commitment(ro.into())
                .call()
                .await
                .unwrap();

            assert_eq!(
                rc_u256
                    .generic_into::<RecordCommitmentSol>()
                    .generic_into::<RecordCommitment>(),
                rc
            );
        }
    }

    #[tokio::test]
    async fn test_derive_record_commitment_checks_reveal_map() {
        let contract = deploy_cape_test().await;
        let mut ro = sol::RecordOpening::default();
        ro.asset_def.policy.reveal_map = U256::from(2).pow(12.into());

        contract
            .derive_record_commitment(ro)
            .call()
            .await
            .should_revert_with_message("Reveal map exceeds 12 bits")
    }

    #[tokio::test]
    async fn test_compute_max_commitments() {
        let contract = deploy_cape_test().await;
        let rng = &mut ark_std::test_rng();

        for _run in 0..10 {
            let mut num_comms = 0;

            let burn_notes = (0..rng.gen_range(0..2))
                .map(|_| {
                    let mut note = sol::BurnNote::default();
                    let n = rng.gen_range(0..10);
                    note.transfer_note.output_commitments = [U256::from(0)].repeat(n);
                    num_comms += n;
                    note
                })
                .collect();

            let transfer_notes = (0..rng.gen_range(0..2))
                .map(|_| {
                    let mut note = sol::TransferNote::default();
                    let n = rng.gen_range(0..10);
                    note.output_commitments = [U256::from(0)].repeat(n);
                    num_comms += n;
                    note
                })
                .collect();

            let freeze_notes = (0..rng.gen_range(0..2))
                .map(|_| {
                    let mut note = sol::FreezeNote::default();
                    let n = rng.gen_range(0..10);
                    note.output_commitments = [U256::from(0)].repeat(n);
                    num_comms += n;
                    note
                })
                .collect();

            let mint_notes = (0..rng.gen_range(0..2))
                .map(|_| {
                    num_comms += 2; // change and mint
                    sol::MintNote::default()
                })
                .collect();

            let cape_block = sol::CapeBlock {
                transfer_notes,
                mint_notes,
                freeze_notes,
                burn_notes,
                note_types: vec![],
                miner_addr: UserPubKey::default().address().into(),
            };

            let max_comms_sol = contract
                .compute_max_commitments(cape_block)
                .call()
                .await
                .unwrap();

            assert_eq!(max_comms_sol, U256::from(num_comms));
        }
    }

    #[tokio::test]
    async fn test_erc20_description() -> Result<()> {
        let contract = deploy_cape_test().await;
        let sponsor = Address::random();
        let asset_address = Address::random();
        let asset_code = Erc20Code(EthereumAddr(asset_address.to_fixed_bytes()));
        let description =
            erc20_asset_description(&asset_code, &EthereumAddr(sponsor.to_fixed_bytes()));
        let ret = contract
            .compute_asset_description(asset_address, sponsor)
            .call()
            .await?;
        assert_eq!(ret.to_vec(), description);
        Ok(())
    }

    #[tokio::test]
    async fn test_check_foreign_asset_code() -> Result<()> {
        let contract = deploy_cape_test().await;

        // Fails for random record opening with random asset code.
        let rng = &mut ark_std::test_rng();
        let ro = RecordOpening::rand_for_test(rng);
        contract
            .check_foreign_asset_code(
                ro.asset_def.code.generic_into::<sol::AssetCodeSol>().0,
                Address::random(),
            )
            .call()
            .await
            .should_revert_with_message("Wrong foreign asset code");

        let erc20_address = Address::random();
        // This is the first account from the test mnemonic
        // TODO define elsewhere to make it usable from other tests
        let sponsor = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".parse::<Address>()?;
        let erc20_code = Erc20Code(EthereumAddr(erc20_address.to_fixed_bytes()));

        // Fails for domestic asset code.
        let domestic_asset_code =
            AssetCode::new_domestic(AssetCodeSeed::generate(rng), erc20_address.as_bytes());
        contract
            .check_foreign_asset_code(
                domestic_asset_code.generic_into::<AssetCodeSol>().0,
                erc20_address,
            )
            .from(sponsor)
            .call()
            .await
            .should_revert_with_message("Wrong foreign asset code");

        // Fails if txn sender address does not match sponsor in asset code.
        let description_wrong_sponsor = erc20_asset_description(
            &erc20_code,
            &EthereumAddr(Address::random().to_fixed_bytes()),
        );
        let asset_code_wrong_sponsor = AssetCode::new_foreign(&description_wrong_sponsor);
        contract
            .check_foreign_asset_code(
                asset_code_wrong_sponsor.generic_into::<AssetCodeSol>().0,
                sponsor,
            )
            .from(sponsor)
            .call()
            .await
            .should_revert_with_message("Wrong foreign asset code");

        let description =
            erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
        let asset_code = AssetCode::new_foreign(&description);

        // Fails for random erc20 address.
        contract
            .check_foreign_asset_code(
                asset_code.generic_into::<sol::AssetCodeSol>().0,
                Address::random(),
            )
            .from(sponsor)
            .call()
            .await
            .should_revert_with_message("Wrong foreign asset code");

        // Passes for correctly derived asset code
        contract
            .check_foreign_asset_code(
                asset_code.generic_into::<sol::AssetCodeSol>().0,
                erc20_address,
            )
            .from(sponsor)
            .call()
            .await
            .should_not_revert();

        Ok(())
    }

    #[tokio::test]
    async fn test_check_domestic_asset_code() -> Result<()> {
        let contract = deploy_cape_test().await;

        // Create a matching pair of codes
        let rng = &mut ark_std::test_rng();
        let description = b"aap_usdx";
        let seed = AssetCodeSeed::generate(rng);
        let internal_asset_code = InternalAssetCode::new(seed, description);
        let asset_code = AssetCode::new_domestic(seed, description);

        // Passes for matching asset codes
        contract
            .check_domestic_asset_code(
                asset_code.generic_into::<AssetCodeSol>().0,
                internal_asset_code.generic_into::<InternalAssetCodeSol>().0,
            )
            .call()
            .await
            .should_not_revert();

        // Fails with non-matching description
        contract
            .check_domestic_asset_code(
                AssetCode::new_domestic(seed, b"other description")
                    .generic_into::<AssetCodeSol>()
                    .0,
                internal_asset_code.generic_into::<InternalAssetCodeSol>().0,
            )
            .call()
            .await
            .should_revert_with_message("Wrong domestic asset code");

        // Fails for foreign asset code
        contract
            .check_domestic_asset_code(
                AssetCode::new_foreign(description)
                    .generic_into::<AssetCodeSol>()
                    .0,
                internal_asset_code.generic_into::<InternalAssetCodeSol>().0,
            )
            .call()
            .await
            .should_revert_with_message("Wrong domestic asset code");

        // Fails if internal asset code doesn't match (different seed)
        contract
            .check_domestic_asset_code(
                asset_code.generic_into::<AssetCodeSol>().0,
                InternalAssetCode::new(AssetCodeSeed::generate(rng), description)
                    .generic_into::<InternalAssetCodeSol>()
                    .0,
            )
            .call()
            .await
            .should_revert_with_message("Wrong domestic asset code");

        Ok(())
    }

    #[tokio::test]
    async fn test_check_domestic_asset_code_in_submit_cape_block() -> Result<()> {
        let contract = deploy_cape_test().await;
        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 0, 1, 0, CapeLedger::merkle_height());

        contract
            .add_root(params.merkle_root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        let mut block = CapeBlock::generate(params.txns, vec![], UserPubKey::default().address())?;

        // Set a wrong internal asset code on the mint note
        block.mint_notes[0].mint_internal_asset_code =
            InternalAssetCode::new(AssetCodeSeed::generate(rng), b"description");

        contract
            .submit_cape_block(block.into(), vec![])
            .call()
            .await
            .should_revert_with_message("Wrong domestic asset code");

        Ok(())
    }

    mod type_conversion {
        use super::*;
        use crate::{ledger::CapeLedger, types::GenericInto};
        use ark_bn254::{Bn254, Fr};
        use ark_std::UniformRand;
        use jf_aap::{
            freeze::FreezeNote,
            mint::MintNote,
            structs::{
                AssetCode, AssetDefinition, AssetPolicy, AuditMemo, Nullifier, RecordCommitment,
                RecordOpening,
            },
            utils::TxnsParams,
            BaseField, NodeValue,
        };
        use jf_plonk::proof_system::structs::Proof;
        use jf_primitives::elgamal;

        async fn deploy_type_contract(
        ) -> Result<TestCapeTypes<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
            let client = get_funded_deployer().await.unwrap();
            let contract = deploy(
                client.clone(),
                Path::new("../abi/contracts/mocks/TestCapeTypes.sol/TestCapeTypes"),
                (),
            )
            .await
            .unwrap();
            Ok(TestCapeTypes::new(contract.address(), client))
        }

        #[tokio::test]
        async fn test_nullifier() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let nf = Nullifier::random_for_test(rng);
                let res = contract
                    .check_nullifier(nf.generic_into::<sol::NullifierSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::NullifierSol>()
                    .generic_into::<Nullifier>();
                assert_eq!(nf, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_record_commitment() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let rc = RecordCommitment::from_field_element(BaseField::rand(rng));
                let res = contract
                    .check_record_commitment(rc.generic_into::<sol::RecordCommitmentSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::RecordCommitmentSol>()
                    .generic_into::<RecordCommitment>();
                assert_eq!(rc, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_merkle_root() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let root = NodeValue::rand(rng);
                let res = contract
                    .check_merkle_root(root.generic_into::<sol::MerkleRootSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::MerkleRootSol>()
                    .generic_into::<NodeValue>();
                assert_eq!(root, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_asset_code() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let (ac, _) = AssetCode::random(rng);
                let res = contract
                    .check_merkle_root(ac.generic_into::<sol::AssetCodeSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::AssetCodeSol>()
                    .generic_into::<AssetCode>();
                assert_eq!(ac, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_asset_policy_and_definition() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                // NOTE: `sol::AssetPolicy` is from abigen! on contract,
                // it collides with `jf_aap::structs::AssetPolicy`
                let policy = AssetPolicy::rand_for_test(rng);
                assert_eq!(
                    policy.clone(),
                    contract
                        .check_asset_policy(policy.generic_into::<sol::AssetPolicy>())
                        .call()
                        .await?
                        .generic_into::<AssetPolicy>()
                );

                let asset_def = AssetDefinition::rand_for_test(rng);
                assert_eq!(
                    asset_def.clone(),
                    contract
                        .check_asset_definition(asset_def.generic_into::<sol::AssetDefinition>())
                        .call()
                        .await?
                        .generic_into::<AssetDefinition>()
                );
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_record_opening() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                // NOTE: `sol::RecordOpening` is from abigen! on contract,
                // it collides with `jf_aap::structs::RecordOpening`
                let ro = RecordOpening::rand_for_test(rng);
                let res = contract
                    .check_record_opening(ro.clone().generic_into::<sol::RecordOpening>())
                    .call()
                    .await?
                    .generic_into::<RecordOpening>();
                assert_eq!(ro.amount, res.amount);
                assert_eq!(ro.asset_def, res.asset_def);
                assert_eq!(ro.pub_key.address(), res.pub_key.address()); // not recovering pub_key.enc_key
                assert_eq!(ro.freeze_flag, res.freeze_flag);
                assert_eq!(ro.blind, res.blind);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_audit_memo() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let keypair = elgamal::KeyPair::generate(rng);
                let message = Fr::rand(rng);
                let ct = keypair.enc_key().encrypt(rng, &[message]);

                let audit_memo = AuditMemo::new(ct);
                assert_eq!(
                    audit_memo.clone(),
                    contract
                        .check_audit_memo(audit_memo.generic_into::<sol::AuditMemo>())
                        .call()
                        .await?
                        .generic_into::<AuditMemo>()
                );
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_plonk_proof_and_txn_notes() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let num_transfer_txn = 1;
            let num_mint_txn = 1;
            let num_freeze_txn = 1;
            let params = TxnsParams::generate_txns(
                rng,
                num_transfer_txn,
                num_mint_txn,
                num_freeze_txn,
                CapeLedger::merkle_height(),
            );

            let contract = deploy_type_contract().await?;
            for txn in params.txns {
                // reconnect with peer
                let client = get_funded_deployer().await?;
                let contract = TestCapeTypes::new(contract.address(), client);

                let proof = txn.validity_proof();
                assert_eq!(
                    proof.clone(),
                    contract
                        .check_plonk_proof(proof.into())
                        .call()
                        .await?
                        .generic_into::<Proof<Bn254>>()
                );

                match txn {
                    TransactionNote::Transfer(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_transfer_note((*note).generic_into::<sol::TransferNote>())
                                .call()
                                .await?
                                .generic_into::<TransferNote>()
                        )
                    }
                    TransactionNote::Mint(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_mint_note((*note).generic_into::<sol::MintNote>())
                                .call()
                                .await?
                                .generic_into::<MintNote>()
                        )
                    }
                    TransactionNote::Freeze(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_freeze_note((*note).generic_into::<sol::FreezeNote>())
                                .call()
                                .await?
                                .generic_into::<FreezeNote>()
                        )
                    }
                }
            }

            Ok(())
        }

        #[tokio::test]
        async fn test_note_type() -> Result<()> {
            let contract = deploy_type_contract().await?;
            let invalid = 10;
            assert_eq!(
                contract
                    .check_note_type(NoteType::Transfer.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                0u8
            );
            assert_eq!(
                contract
                    .check_note_type(NoteType::Mint.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                1u8
            );
            assert_eq!(
                contract
                    .check_note_type(NoteType::Freeze.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                2u8
            );

            assert_eq!(
                contract
                    .check_note_type(NoteType::Burn.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                3u8
            );

            Ok(())
        }
    }
}
