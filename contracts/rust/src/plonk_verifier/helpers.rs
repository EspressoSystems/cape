use anyhow::Result;
use ark_bn254::{Bn254, Fq, Fr};
use ark_ff::PrimeField;
use ark_std::{convert::TryInto, test_rng};
use itertools::izip;
use jf_plonk::{
    circuit::{Arithmetization, Circuit, PlonkCircuit},
    proof_system::{
        structs::{Proof, ProofEvaluations, VerifyingKey},
        PlonkKzgSnark, Snark,
    },
    transcript::SolidityTranscript,
};
use jf_utils::fq_to_fr;

/// return list of (proof, ver_key, public_input, extra_msg, domain_size)
pub(crate) fn gen_plonk_proof_for_test(
    num_proof: usize,
) -> Result<
    Vec<(
        Proof<Bn254>,
        VerifyingKey<Bn254>,
        Vec<Fr>,
        Option<Vec<u8>>,
        usize,
    )>,
> {
    // 1. Simulate universal setup
    let rng = &mut test_rng();
    let n = 64;
    let max_degree = n + 2;
    let srs = PlonkKzgSnark::<Bn254>::universal_setup(max_degree, rng)?;

    // 2. Create circuits
    let circuits = (0..num_proof)
        .map(|i| {
            let m = 2 + i / 3;
            let a0 = 1 + i % 3;
            gen_circuit_for_test::<Fr>(m, a0)
        })
        .collect::<Result<Vec<_>>>()?;
    let domain_sizes: Vec<usize> = circuits
        .iter()
        .map(|c| c.eval_domain_size().unwrap())
        .collect();

    // 3. Preprocessing
    let mut prove_keys = vec![];
    let mut ver_keys = vec![];
    for c in circuits.iter() {
        let (pk, vk) = PlonkKzgSnark::<Bn254>::preprocess(&srs, c)?;
        prove_keys.push(pk);
        ver_keys.push(vk);
    }

    // 4. Proving
    let mut proofs = vec![];
    let mut extra_msgs = vec![];

    circuits
        .iter()
        .zip(prove_keys.iter())
        .enumerate()
        .for_each(|(i, (cs, pk))| {
            let extra_msg = if i % 2 == 0 {
                None
            } else {
                Some(format!("extra message: {}", i).into_bytes())
            };
            proofs.push(
                PlonkKzgSnark::<Bn254>::prove::<_, _, SolidityTranscript>(
                    rng,
                    cs,
                    &pk,
                    extra_msg.clone(),
                )
                .unwrap(),
            );
            extra_msgs.push(extra_msg);
        });

    let public_inputs: Vec<Vec<Fr>> = circuits
        .iter()
        .map(|cs| cs.public_input().unwrap())
        .collect();

    Ok(izip!(proofs, ver_keys, public_inputs, extra_msgs, domain_sizes).collect())
}

// Different `m`s lead to different circuits.
// Different `a0`s lead to different witness values.
// For UltraPlonk circuits, `a0` should be less than or equal to `m+1`
#[allow(dead_code)]
fn gen_circuit_for_test<F: PrimeField>(m: usize, a0: usize) -> Result<PlonkCircuit<F>> {
    let mut cs: PlonkCircuit<F> = PlonkCircuit::new_turbo_plonk();
    // Create variables
    let mut a = vec![];
    for i in a0..(a0 + 4 * m) {
        a.push(cs.create_variable(F::from(i as u64))?);
    }
    let b = vec![
        cs.create_public_variable(F::from(m as u64 * 2))?,
        cs.create_public_variable(F::from(a0 as u64 * 2 + m as u64 * 4 - 1))?,
    ];
    let c = cs.create_public_variable(
        (cs.witness(b[1])? + cs.witness(a[0])?) * (cs.witness(b[1])? - cs.witness(a[0])?),
    )?;

    // Create gates:
    // 1. a0 + ... + a_{4*m-1} = b0 * b1
    // 2. (b1 + a0) * (b1 - a0) = c
    // 3. b0 = 2 * m
    let mut acc = cs.zero();
    a.iter().for_each(|&elem| acc = cs.add(acc, elem).unwrap());
    let b_mul = cs.mul(b[0], b[1])?;
    cs.equal_gate(acc, b_mul)?;
    let b1_plus_a0 = cs.add(b[1], a[0])?;
    let b1_minus_a0 = cs.sub(b[1], a[0])?;
    cs.mul_gate(b1_plus_a0, b1_minus_a0, c)?;
    cs.constant_gate(b[0], F::from(m as u64 * 2))?;

    // Finalize the circuit.
    cs.finalize_for_arithmetization()?;

    Ok(cs)
}

// getter of polynomial evaluations of `proof`, since the fields visibility is not public
pub(crate) fn get_poly_evals(proof: Proof<Bn254>) -> ProofEvaluations<Fr> {
    const NUM_G1_POINT: usize = 13;
    let mut scalars: Vec<Fq> = proof.into();
    let poly_evals_scalars: Vec<Fr> = scalars
        .drain((2 * NUM_G1_POINT)..)
        .map(|fq| fq_to_fr::<Fq, ark_bn254::g1::Parameters>(&fq))
        .collect();
    poly_evals_scalars.try_into().unwrap()
}
