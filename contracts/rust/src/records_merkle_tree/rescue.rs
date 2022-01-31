#[cfg(test)]
mod tests {

    use crate::ethereum;
    use crate::helpers::{convert_fr254_to_u256, convert_u256_to_bytes_le};
    use crate::types::TestRescue;
    use ark_ed_on_bn254::Fq as Fr254;
    use ark_ff::{BigInteger, PrimeField, UniformRand, Zero};
    use ethers::prelude::k256::ecdsa::SigningKey;
    use ethers::prelude::*;
    use jf_rescue::Permutation;
    use std::default::Default;
    use std::path::Path;

    // From jellyfish rescue/src/lib.rs
    // hash output on vector [0, 0, 0, 0]
    // this value is cross checked with sage script
    // first three vectors of rescue254.Sponge([0,0,0,0], 4)
    pub(crate) const OUTPUT254: [[u8; 32]; 3] = [
        [
            0xDD, 0xE7, 0x55, 0x8E, 0x14, 0xF9, 0x4C, 0xEE, 0x9F, 0xCC, 0xB2, 0x02, 0xFC, 0x0E,
            0x54, 0x21, 0xF2, 0xAA, 0xB8, 0x48, 0x05, 0xDB, 0x9B, 0x7A, 0xD2, 0x36, 0xA5, 0xF1,
            0x49, 0x77, 0xB4, 0x17,
        ],
        [
            0x43, 0x5F, 0x99, 0x3C, 0xB7, 0xB3, 0x84, 0x74, 0x4E, 0x80, 0x83, 0xFF, 0x73, 0x20,
            0x07, 0xD9, 0x7B, 0xEC, 0x4B, 0x90, 0x48, 0x1D, 0xFD, 0x72, 0x4C, 0xF0, 0xA5, 0x7C,
            0xDC, 0x68, 0xC0, 0x25,
        ],
        [
            0x2C, 0x7B, 0x21, 0x09, 0x9D, 0x10, 0xE9, 0x5C, 0x36, 0x3E, 0x6D, 0x20, 0x28, 0xBB,
            0xDB, 0x1E, 0xED, 0xF4, 0x22, 0x9B, 0x3A, 0xEE, 0x1E, 0x6F, 0x89, 0x13, 0x3D, 0x1E,
            0x4C, 0xA0, 0xA6, 0x23,
        ],
    ];

    #[test]
    fn test_rescue_sponge_jellyfish() {
        let rescue = Permutation::default();
        let input = [Fr254::zero(); 3];
        let expected = vec![
            Fr254::from_le_bytes_mod_order(&OUTPUT254[0]),
            Fr254::from_le_bytes_mod_order(&OUTPUT254[1]),
            Fr254::from_le_bytes_mod_order(&OUTPUT254[2]),
        ];
        let real_output = rescue.sponge_no_padding(&input, 3).unwrap();
        assert_eq!(real_output, expected);
    }

    async fn deploy_test_rescue() -> TestRescue<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>
    {
        let client = ethereum::get_funded_client().await.unwrap();
        let contract = ethereum::deploy(
            client.clone(),
            Path::new("../abi/contracts/mocks/TestRescue.sol/TestRescue"),
            (),
        )
        .await
        .unwrap();
        TestRescue::new(contract.address(), client)
    }

    #[tokio::test]
    async fn test_rescue_hash_function_solidity_for_zero_vector_input() {
        let contract = deploy_test_rescue().await;

        let res: U256 = contract
            .hash(U256::from(0), U256::from(0), U256::from(0))
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(convert_u256_to_bytes_le(res).as_slice(), OUTPUT254[0]);
    }

    #[tokio::test]
    async fn test_rescue_hash_function_solidity_on_random_inputs() {
        let contract = deploy_test_rescue().await;

        let rescue = Permutation::default();
        let mut rng = ark_std::test_rng();

        for _ in 0..10 {
            let input1 = Fr254::rand(&mut rng);
            let input2 = Fr254::rand(&mut rng);
            let input3 = Fr254::rand(&mut rng);

            let input = [input1, input2, input3];

            let input1_u256 = convert_fr254_to_u256(input1);
            let input2_u256 = convert_fr254_to_u256(input2);
            let input3_u256 = convert_fr254_to_u256(input3);

            let res_fr254 = rescue.sponge_no_padding(&input, 1).unwrap();

            let res_u256: U256 = contract
                .hash(input1_u256, input2_u256, input3_u256)
                .call()
                .await
                .unwrap()
                .into();

            assert_eq!(
                convert_u256_to_bytes_le(res_u256).as_slice(),
                res_fr254[0].into_repr().to_bytes_le()
            );
        }
    }
}
