use crate::types as sol;
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
use zerok_lib::cape_state::CapeTransaction;

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
pub(crate) struct CAPEConstructorArgs {
    height: u8,
    n_roots: u64,
}

#[allow(dead_code)]
impl CAPEConstructorArgs {
    pub(crate) fn new(height: u8, n_roots: u64) -> Self {
        Self { height, n_roots }
    }
}

impl From<CAPEConstructorArgs> for (u8, u64) {
    fn from(args: CAPEConstructorArgs) -> (u8, u64) {
        (args.height, args.n_roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::prelude::k256::ecdsa::SigningKey;
    use ethers::prelude::{Address, Http, Provider, SignerMiddleware, Wallet, U256};
    use jf_aap::keys::UserKeyPair;
    use jf_aap::structs::{AssetDefinition, FreezeFlag, RecordOpening};
    use jf_aap::MerkleTree;
    use jf_aap::TransactionVerifyingKey;
    use rand::Rng;
    use zerok_lib::cape_state::{CapeEthEffect, CapeEvent, CapeOperation};

    use crate::assertion::Matcher;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::types::field_to_u256;
    use crate::types::{
        GenericInto, MerkleRootSol, NullifierSol, RecordCommitmentSol, TestCAPE, TestCapeTypes,
    };
    use anyhow::Result;
    use jf_aap::keys::UserPubKey;
    use jf_aap::transfer::TransferNoteInput;
    use jf_aap::utils::TxnsParams;
    use jf_aap::AccMemberWitness;
    use jf_aap::MerkleLeafProof;
    use jf_utils::CanonicalBytes;
    use rand_chacha::ChaChaRng;
    use std::env;
    use std::path::Path;
    use std::time::Instant;
    use zerok_lib::cape_state::CapeContractState;
    use zerok_lib::state::ProverKeySet;
    use zerok_lib::state::{key_set, key_set::KeySet, VerifierKeySet, MERKLE_HEIGHT};
    use zerok_lib::universal_params::UNIVERSAL_PARAM;
    // use zerok_lib::util::canonical;
    use rand::SeedableRng;

    async fn deploy_cape_test() -> TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
        let client = get_funded_deployer().await.unwrap();
        let call = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
            CAPEConstructorArgs::new(5, 2).generic_into::<(u8, u64)>(),
        )
        .await;
        let contract = call.unwrap();
        TestCAPE::new(contract.address(), client)
    }

    #[tokio::test]
    async fn test_2user() -> Result<()> {
        let now = Instant::now();

        println!("generating params");

        let mut prng = ChaChaRng::from_seed([0x8au8; 32]);

        let univ_setup = &*UNIVERSAL_PARAM;

        let (xfr_prove_key, xfr_verif_key, _) =
            jf_aap::proof::transfer::preprocess(univ_setup, 1, 2, MERKLE_HEIGHT).unwrap();
        let (mint_prove_key, mint_verif_key, _) =
            jf_aap::proof::mint::preprocess(univ_setup, MERKLE_HEIGHT).unwrap();
        let (freeze_prove_key, freeze_verif_key, _) =
            jf_aap::proof::freeze::preprocess(univ_setup, 2, MERKLE_HEIGHT).unwrap();

        for (l, k) in vec![
            ("xfr", CanonicalBytes::from(xfr_verif_key.clone())),
            ("mint", CanonicalBytes::from(mint_verif_key.clone())),
            ("freeze", CanonicalBytes::from(freeze_verif_key.clone())),
        ] {
            println!("{}: {} bytes", l, k.0.len());
        }

        let prove_keys = ProverKeySet::<key_set::OrderByInputs> {
            mint: mint_prove_key,
            xfr: KeySet::new(vec![xfr_prove_key].into_iter()).unwrap(),
            freeze: KeySet::new(vec![freeze_prove_key].into_iter()).unwrap(),
        };

        let verif_keys = VerifierKeySet {
            mint: TransactionVerifyingKey::Mint(mint_verif_key),
            xfr: KeySet::new(vec![TransactionVerifyingKey::Transfer(xfr_verif_key)].into_iter())
                .unwrap(),
            freeze: KeySet::new(
                vec![TransactionVerifyingKey::Freeze(freeze_verif_key)].into_iter(),
            )
            .unwrap(),
        };

        println!("CRS set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let alice_key = UserKeyPair::generate(&mut prng);
        let bob_key = UserKeyPair::generate(&mut prng);

        let coin = AssetDefinition::native();

        let alice_rec_builder = RecordOpening::new(
            &mut prng,
            2,
            coin.clone(),
            alice_key.pub_key(),
            FreezeFlag::Unfrozen,
        );

        let alice_rec1 = alice_rec_builder;

        let mut t = MerkleTree::new(MERKLE_HEIGHT).unwrap();
        assert_eq!(
            t.commitment(),
            MerkleTree::new(MERKLE_HEIGHT).unwrap().commitment()
        );
        let alice_rec_elem = RecordCommitment::from(&alice_rec1);
        // dbg!(&RecordCommitment::from(&alice_rec1));
        assert_eq!(
            RecordCommitment::from(&alice_rec1),
            RecordCommitment::from(&alice_rec1)
        );
        t.push(RecordCommitment::from(&alice_rec1).to_field_element());
        let alice_rec_path = t.get_leaf(0).expect_ok().unwrap().1.path;
        assert_eq!(alice_rec_path.nodes.len(), MERKLE_HEIGHT as usize);

        println!("Tree set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let first_root = t.commitment().root_value;

        let alice_rec_final = TransferNoteInput {
            ro: alice_rec1,
            owner_keypair: &alice_key,
            cred: None,
            acc_member_witness: AccMemberWitness {
                merkle_path: alice_rec_path.clone(),
                root: first_root,
                uid: 0,
            },
        };

        let mut wallet_merkle_tree = t.clone();
        let validator = CapeContractState::new(verif_keys, t);

        println!("Validator set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        MerkleTree::check_proof(
            validator.ledger.record_merkle_commitment.root_value,
            0,
            &MerkleLeafProof::new(alice_rec_elem.to_field_element(), alice_rec_path),
        )
        .unwrap();

        println!("Path checked: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let ((txn1, _, _), bob_rec) = {
            let bob_rec = RecordOpening::new(
                &mut prng,
                1, /* 1 less, for the transaction fee */
                coin,
                bob_key.pub_key(),
                FreezeFlag::Unfrozen,
            );

            let txn = TransferNote::generate_native(
                &mut prng,
                /* inputs:         */ vec![alice_rec_final],
                /* outputs:        */ &[bob_rec.clone()],
                /* fee:            */ 1,
                /* valid_until:    */ 2,
                /* proving_key:    */ prove_keys.xfr.key_for_size(1, 2).unwrap(),
            )
            .unwrap();
            (txn, bob_rec)
        };

        println!("Transfer has {} outputs", txn1.output_commitments.len());
        // println!(
        //     "Transfer is {} bytes long",
        //     canonical::serialize(&txn1).unwrap().len()
        // );

        println!("Transfer generated: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let new_recs = txn1.output_commitments.to_vec();

        let txn1_cape = CapeTransaction::AAP(TransactionNote::Transfer(Box::new(txn1)));

        let (new_state, effects) = validator
            .submit_operations(vec![CapeOperation::SubmitBlock(vec![txn1_cape.clone()])])
            .unwrap();

        println!(
            "Transfer validated & applied: {}s",
            now.elapsed().as_secs_f32()
        );
        let now = Instant::now();

        assert_eq!(effects.len(), 1);
        if let CapeEthEffect::Emit(CapeEvent::BlockCommitted {
            wraps: wrapped_commitments,
            txns: filtered_txns,
        }) = effects[0].clone()
        {
            assert_eq!(wrapped_commitments, vec![]);
            assert_eq!(filtered_txns.len(), 1);
            assert_eq!(filtered_txns[0], txn1_cape);
        } else {
            panic!("Transaction emitted the wrong event!");
        }

        for r in new_recs {
            wallet_merkle_tree.push(r.to_field_element());
        }

        assert_eq!(
            new_state.ledger.record_merkle_commitment,
            wallet_merkle_tree.commitment()
        );

        let _bob_rec = TransferNoteInput {
            ro: bob_rec,
            owner_keypair: &bob_key,
            cred: None,
            acc_member_witness: AccMemberWitness {
                merkle_path: wallet_merkle_tree.get_leaf(2).expect_ok().unwrap().1.path,
                root: new_state.ledger.record_merkle_commitment.root_value,
                uid: 2,
            },
        };

        println!(
            "New record merkle path retrieved: {}s",
            now.elapsed().as_secs_f32()
        );
        println!("Old state: {:?}", validator);
        println!("New state: {:?}", new_state);
        Ok(())
    }

    #[tokio::test]
    async fn test_2user_and_submit() -> Result<()> {
        let now = Instant::now();

        println!("generating params");

        let mut prng = ChaChaRng::from_seed([0x8au8; 32]);

        let univ_setup = &*UNIVERSAL_PARAM;

        let (xfr_prove_key, xfr_verif_key, _) =
            jf_aap::proof::transfer::preprocess(univ_setup, 1, 2, MERKLE_HEIGHT).unwrap();
        let (mint_prove_key, mint_verif_key, _) =
            jf_aap::proof::mint::preprocess(univ_setup, MERKLE_HEIGHT).unwrap();
        let (freeze_prove_key, freeze_verif_key, _) =
            jf_aap::proof::freeze::preprocess(univ_setup, 2, MERKLE_HEIGHT).unwrap();

        for (l, k) in vec![
            ("xfr", CanonicalBytes::from(xfr_verif_key.clone())),
            ("mint", CanonicalBytes::from(mint_verif_key.clone())),
            ("freeze", CanonicalBytes::from(freeze_verif_key.clone())),
        ] {
            println!("{}: {} bytes", l, k.0.len());
        }

        let prove_keys = ProverKeySet::<key_set::OrderByInputs> {
            mint: mint_prove_key,
            xfr: KeySet::new(vec![xfr_prove_key].into_iter()).unwrap(),
            freeze: KeySet::new(vec![freeze_prove_key].into_iter()).unwrap(),
        };

        let verif_keys = VerifierKeySet {
            mint: TransactionVerifyingKey::Mint(mint_verif_key),
            xfr: KeySet::new(vec![TransactionVerifyingKey::Transfer(xfr_verif_key)].into_iter())
                .unwrap(),
            freeze: KeySet::new(
                vec![TransactionVerifyingKey::Freeze(freeze_verif_key)].into_iter(),
            )
            .unwrap(),
        };

        println!("CRS set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let client = get_funded_deployer().await.unwrap();

        let contract_address: Address = match env::var("CAPE_ADDRESS") {
            Ok(val) => val.parse::<Address>().unwrap(),
            Err(_) => deploy(
                client.clone(),
                // TODO using mock contract to be able to manually add root
                Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
                CAPEConstructorArgs::new(
                    MERKLE_HEIGHT,
                    CapeContractState::RECORD_ROOT_HISTORY_SIZE as u64,
                )
                .generic_into::<(u8, u64)>(),
            )
            .await
            .unwrap()
            .address(),
        };

        let contract = TestCAPE::new(contract_address, client);

        println!("Contract set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let alice_key = UserKeyPair::generate(&mut prng);
        let bob_key = UserKeyPair::generate(&mut prng);

        let coin = AssetDefinition::native();

        let alice_rec_builder = RecordOpening::new(
            &mut prng,
            2,
            coin.clone(),
            alice_key.pub_key(),
            FreezeFlag::Unfrozen,
        );

        let alice_rec1 = alice_rec_builder;

        let mut t = MerkleTree::new(MERKLE_HEIGHT).unwrap();
        assert_eq!(
            t.commitment(),
            MerkleTree::new(MERKLE_HEIGHT).unwrap().commitment()
        );
        let alice_rec_elem = RecordCommitment::from(&alice_rec1);
        // dbg!(&RecordCommitment::from(&alice_rec1));
        assert_eq!(
            RecordCommitment::from(&alice_rec1),
            RecordCommitment::from(&alice_rec1)
        );
        let alice_rec_field_elem = RecordCommitment::from(&alice_rec1).to_field_element();
        t.push(alice_rec_field_elem);
        let alice_rec_path = t.get_leaf(0).expect_ok().unwrap().1.path;
        assert_eq!(alice_rec_path.nodes.len(), MERKLE_HEIGHT as usize);

        contract
            .test_only_insert_record_commitments(vec![field_to_u256(alice_rec_field_elem)])
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        let first_root = t.commitment().root_value;

        assert_eq!(
            contract.test_only_get_num_leaves().call().await.unwrap(),
            U256::from(1)
        );

        assert_eq!(
            contract.get_root_value().call().await.unwrap(),
            field_to_u256(first_root.to_scalar())
        );

        println!("Tree set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let alice_rec_final = TransferNoteInput {
            ro: alice_rec1,
            owner_keypair: &alice_key,
            cred: None,
            acc_member_witness: AccMemberWitness {
                merkle_path: alice_rec_path.clone(),
                root: first_root,
                uid: 0,
            },
        };

        let mut wallet_merkle_tree = t.clone();
        let validator = CapeContractState::new(verif_keys, t);

        println!("Validator set up: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        MerkleTree::check_proof(
            validator.ledger.record_merkle_commitment.root_value,
            0,
            &MerkleLeafProof::new(alice_rec_elem.to_field_element(), alice_rec_path),
        )
        .unwrap();

        println!("Path checked: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let ((txn1, _, _), bob_rec) = {
            let bob_rec = RecordOpening::new(
                &mut prng,
                1, /* 1 less, for the transaction fee */
                coin,
                bob_key.pub_key(),
                FreezeFlag::Unfrozen,
            );

            let txn = TransferNote::generate_native(
                &mut prng,
                /* inputs:         */ vec![alice_rec_final],
                /* outputs:        */ &[bob_rec.clone()],
                /* fee:            */ 1,
                /* valid_until:    */ 2,
                /* proving_key:    */ prove_keys.xfr.key_for_size(1, 2).unwrap(),
            )
            .unwrap();
            (txn, bob_rec)
        };

        println!("Transfer has {} outputs", txn1.output_commitments.len());
        // println!(
        //     "Transfer is {} bytes long",
        //     canonical::serialize(&txn1).unwrap().len()
        // );

        println!("Transfer generated: {}s", now.elapsed().as_secs_f32());
        let now = Instant::now();

        let new_recs = txn1.output_commitments.to_vec();

        let txn1_cape = CapeTransaction::AAP(TransactionNote::Transfer(Box::new(txn1)));

        let (new_state, effects) = validator
            .submit_operations(vec![CapeOperation::SubmitBlock(vec![txn1_cape.clone()])])
            .unwrap();

        let miner = UserPubKey::default();
        let cape_block =
            CapeBlock::from_cape_transactions(vec![txn1_cape.clone()], miner.address())?;
        // Submit to the contract
        contract
            .submit_cape_block(cape_block.into(), vec![])
            .send()
            .await?
            .await?;

        println!(
            "Transfer validated & applied: {}s",
            now.elapsed().as_secs_f32()
        );
        let now = Instant::now();

        assert_eq!(effects.len(), 1);
        if let CapeEthEffect::Emit(CapeEvent::BlockCommitted {
            wraps: wrapped_commitments,
            txns: filtered_txns,
        }) = effects[0].clone()
        {
            assert_eq!(wrapped_commitments, vec![]);
            assert_eq!(filtered_txns.len(), 1);
            assert_eq!(filtered_txns[0], txn1_cape);
        } else {
            panic!("Transaction emitted the wrong event!");
        }

        for r in new_recs {
            wallet_merkle_tree.push(r.to_field_element());
        }

        assert_eq!(
            new_state.ledger.record_merkle_commitment,
            wallet_merkle_tree.commitment()
        );

        let _bob_rec = TransferNoteInput {
            ro: bob_rec,
            owner_keypair: &bob_key,
            cred: None,
            acc_member_witness: AccMemberWitness {
                merkle_path: wallet_merkle_tree.get_leaf(2).expect_ok().unwrap().1.path,
                root: new_state.ledger.record_merkle_commitment.root_value,
                uid: 2,
            },
        };

        println!(
            "New record merkle path retrieved: {}s",
            now.elapsed().as_secs_f32()
        );
        println!("Old state: {:?}", validator);
        println!("New state: {:?}", new_state);
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_block_to_cape_contract() -> Result<()> {
        let client = get_funded_deployer().await.unwrap();

        let contract_address: Address = match env::var("CAPE_ADDRESS") {
            Ok(val) => val.parse::<Address>().unwrap(),
            Err(_) => deploy(
                client.clone(),
                // TODO using mock contract to be able to manually add root
                Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
                CAPEConstructorArgs::new(5, 2).generic_into::<(u8, u64)>(),
            )
            .await
            .unwrap()
            .address(),
        };

        let contract = TestCAPE::new(contract_address, client);

        // Create two transactions
        let rng = &mut ark_std::test_rng();
        let num_transfer_txn = 1;
        let num_mint_txn = 1;
        let num_freeze_txn = 1;
        let params = TxnsParams::generate_txns(rng, num_transfer_txn, num_mint_txn, num_freeze_txn);
        let miner = UserPubKey::default();

        let nf = params.txns[0].nullifiers()[0];
        let root = params.txns[0].merkle_root();

        // temporarily no burn txn yet.
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        // Check that some nullifier is not yet inserted
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
        assert!(call.should_revert_with_message("Bad burn tag"));
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
        assert!(call.should_revert_with_message("Bad burn destination"));
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
        assert!(call.should_revert_with_message("Bad record commitment"));
    }

    // TODO Add test for check_burn that passes

    // TODO integration test to check if check_burn is hooked up correctly in
    // main block validaton loop.

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
        assert!(call.should_revert_with_message("Burn prefix in transfer note"));
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

        assert!(contract
            .derive_record_commitment(ro)
            .call()
            .await
            .should_revert_with_message("Reveal map exceeds 12 bits"))
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

    mod type_conversion {
        use super::*;
        use crate::types::GenericInto;
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
                Path::new("../artifacts/contracts/mocks/TestCapeTypes.sol/TestCapeTypes"),
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
            let contract = deploy_type_contract().await?;
            let num_transfer_txn = 1;
            let num_mint_txn = 1;
            let num_freeze_txn = 1;
            let params =
                TxnsParams::generate_txns(rng, num_transfer_txn, num_mint_txn, num_freeze_txn);

            for txn in params.txns {
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
