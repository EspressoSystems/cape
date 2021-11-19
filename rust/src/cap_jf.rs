use jf_txn::{
    keys::UserKeyPair,
    proof::{transfer::preprocess, universal_setup},
    structs::NoteType,
    transfer::TransferNote,
    utils::{compute_universal_param_size, params_builder::TransferParamsBuilder},
};

pub fn create_test_anon_xfr_2in_6out() -> TransferNote {
    let depth = 10;
    let num_input = 2;
    let num_output = 6;
    let cred_expiry = 9999;
    let valid_until = 1234;

    let mut prng = ark_std::test_rng();
    let domain_size =
        compute_universal_param_size(NoteType::Transfer, num_input, num_output, depth).unwrap();
    let srs = universal_setup(domain_size, &mut prng).unwrap();
    let (prover_key, _verifier_key, _) = preprocess(&srs, num_input, num_output, depth).unwrap();

    let input_amounts = [30, 25];
    let output_amounts = [19, 3, 4, 5, 6, 7];

    let keypair1 = UserKeyPair::generate(&mut prng);
    let keypair2 = UserKeyPair::generate(&mut prng);
    let builder = TransferParamsBuilder::new_non_native(
        num_input,
        num_output,
        Some(depth),
        vec![&keypair1, &keypair2],
    )
    .set_input_amounts(input_amounts[0], &input_amounts[1..])
    .set_output_amounts(output_amounts[0], &output_amounts[1..])
    .set_input_creds(cred_expiry);

    let (note, _recv_memos, _sig) = builder
        .build_transfer_note(&mut prng, &prover_key, valid_until, vec![])
        .unwrap();

    note
}
