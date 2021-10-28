const { ethers } = require("hardhat");
const common = require("../lib/common");

async function print_report(
  owner,
  title,
  fun_to_eval,
  fun_names,
  chunk,
  merkle_tree_update,
  is_starkware
) {
  console.log("**** " + title + "****");
  for (let i = 0; i < fun_to_eval.length; i++) {
    let res;
    try {
      res = await common.compute_gas_and_price(
        owner,
        fun_to_eval[i],
        chunk,
        merkle_tree_update,
        is_starkware
      );
      let gas = res[0];

      let price = res[1].getValue();
      console.log(
        fun_names[i] + ":  " + gas + " gas  ------ " + price + " USD "
      );
    } catch (error) {
      console.log(error);
    }
  }
  console.log("\n");
}

async function main() {
  const [owner] = await ethers.getSigners();
  let fun_to_eval, fun_names;

  const DPV = await ethers.getContractFactory("DummyVerifier");
  const dpv = await DPV.deploy();

  // Polling interval in ms.
  dpv.provider.pollingInterval = 20;

  await dpv.deployed();

  console.log("Contract deployed at address " + dpv.address);

  fun_to_eval = [dpv.verify_empty];
  fun_names = ["verify_empty"];

  const N_APPT_TX_MAX = 10;
  for (let i = 1; i < N_APPT_TX_MAX; i++) {
    const chunk = common.create_chunk(i);

    await print_report(
      owner,
      "Empty inputs",
      fun_to_eval,
      fun_names,
      chunk,
      false,
      false
    );
  }
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
