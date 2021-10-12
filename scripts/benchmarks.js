const { ethers } = require("hardhat");
const common  = require("../lib/common");

const N_AAPTX = 1;

const GAS_PRICE = 223; // See https://legacy.ethgasstation.info/calculatorTxV.php
const ETH_PRICE_USD = 3388;

async function compute_gas_and_price(
    fun_to_evaluate,
    chunk,
    merkle_trees_update,
    is_starkware,
) {
    const tx = await fun_to_evaluate(chunk, merkle_trees_update, is_starkware);
    const txReceipt = await tx.wait();

    const gasUsed = txReceipt.gasUsed.toString();
    const price = gasUsed * GAS_PRICE * 10**(-9) * ETH_PRICE_USD;
    return [gasUsed,price];
}

async function print_report(title,fun_to_eval,fun_names,chunk,merkle_tree_update,is_starkware) {
    console.log("**** " + title + "****");
    for (let i = 0; i < fun_to_eval.length; i++) {
        let res;
        try {
            res = await compute_gas_and_price(fun_to_eval[i], chunk, merkle_tree_update, is_starkware);
            let gas = res[0] / N_AAPTX;
            let price = res[1] / N_AAPTX;
            console.log(fun_names[i] + ":  " + gas + " gas  ------ " +  price + " USD ");
        } catch(error) {
            console.log(error);
        }
    }
    console.log("\n");
}

async function main() {

    let fun_to_eval, fun_names;

    const DPV = await ethers.getContractFactory("DummyVerifier");
    const dpv = await DPV.deploy();

    // Polling interval in ms.
    dpv.provider.pollingInterval = 20;

    await dpv.deployed();

    console.log("Contract deployed at address " + dpv.address);

    fun_to_eval = [dpv.verify_empty, dpv.verify, dpv.batch_verify];
    fun_names = ["verify_empty", "verify", "batch_verify"];

    const chunk = common.create_chunk(N_AAPTX);

    await print_report("NO Merkle tree update",fun_to_eval,fun_names,chunk,false,false);

    await print_report("Merkle tree update (Starkware)",fun_to_eval,fun_names,chunk,true,true);

    await print_report("Merkle tree update (NO Starkware)",fun_to_eval,fun_names,chunk,true,false);
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
