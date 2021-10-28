const { ethers } = require("hardhat");
const common = require("../lib/common");
const bigDecimal = require("js-big-decimal");

const N_AAPTX = 1;

const ETH_PRICE_USD = 3962;

async function compute_gas_and_price(
  owner,
  fun_to_evaluate,
  chunk,
  merkle_trees_update,
  is_starkware
) {
  // Balance before
  let balance_before = await owner.getBalance();

  const tx = await fun_to_evaluate(chunk, merkle_trees_update, is_starkware);
  const txReceipt = await tx.wait();
  const gasUsed = txReceipt.gasUsed.toString();

  // Balance after
  let balance_after = await owner.getBalance();
  let wei_cost = BigInt(balance_before) - BigInt(balance_after);

  const WEI_PER_ETH = 10 ** 18;
  const wei_cost_big_decimal = new bigDecimal(wei_cost.toString());
  const eth_price_usd_big_decimal = new bigDecimal(ETH_PRICE_USD.toString());
  const wei_per_eth_big_decimal = new bigDecimal(WEI_PER_ETH.toString());
  const price = wei_cost_big_decimal
    .multiply(eth_price_usd_big_decimal)
    .divide(wei_per_eth_big_decimal);
  return [gasUsed, price];
}

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
      res = await compute_gas_and_price(
        owner,
        fun_to_eval[i],
        chunk,
        merkle_tree_update,
        is_starkware
      );
      let gas = res[0] / N_AAPTX;
      let N_APPTX_BIG_DECIMAL = new bigDecimal(N_AAPTX);
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

  const DPV = await ethers.getContractFactory("DummyVerifier");
  const dpv = await DPV.deploy();

  // Polling interval in ms.
  dpv.provider.pollingInterval = 20;

  await dpv.deployed();

  console.log("Contract deployed at address " + dpv.address);

  fun_to_eval = [dpv.verify_empty, dpv.verify, dpv.batch_verify];
  fun_names = ["verify_empty", "verify", "batch_verify"];

  const chunk = common.create_chunk(N_AAPTX);

  await print_report(
    owner,
    "NO Merkle tree update",
    fun_to_eval,
    fun_names,
    chunk,
    false,
    false
  );

  await print_report(
    owner,
    "Merkle tree update (Starkware)",
    fun_to_eval,
    fun_names,
    chunk,
    true,
    true
  );

  await print_report(
    owner,
    "Merkle tree update (NO Starkware)",
    fun_to_eval,
    fun_names,
    chunk,
    true,
    false
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
