const { ethers } = require("hardhat");
const common = require("../lib/common");
const bigDecimal = require("js-big-decimal");

const N_CAPTX = 1;

async function print_report(
  owner,
  title,
  fun_to_eval,
  fun_names,
  chunk,
  merkle_tree_update
) {
  console.log("**** " + title + " ****");
  for (let i = 0; i < fun_to_eval.length; i++) {
    let res;
    try {
      res = await common.compute_gas_and_price(owner, fun_to_eval[i], [
        chunk,
        merkle_tree_update,
      ]);
      let gas = res[0] / N_CAPTX;
      let N_APPTX_BIG_DECIMAL = new bigDecimal(N_CAPTX);
      let price = res[1].divide(N_APPTX_BIG_DECIMAL).getValue();
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

  const DPV = await ethers.getContractFactory("DummyCAPE");
  const dpv = await DPV.deploy();

  // Polling interval in ms.
  dpv.provider.pollingInterval = 20;

  await dpv.deployed();

  console.log("Contract deployed at address " + dpv.address);

  fun_to_eval = [dpv.verifyEmpty, dpv.verify, dpv.batchVerify];
  fun_names = ["verifyEmpty", "verify", "batchVerify"];

  const chunk = common.create_chunk(N_CAPTX);

  await print_report(
    owner,
    "NO Merkle tree update",
    fun_to_eval,
    fun_names,
    chunk,
    false
  );

  await print_report(
    owner,
    "Merkle tree update",
    fun_to_eval,
    fun_names,
    chunk,
    true
  );
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
