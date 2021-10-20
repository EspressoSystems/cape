const { ethers } = require("hardhat");
const { BigNumber, utils } = ethers;
const common = require("../lib/common");

const N_AAPTX = 1;

const GAS_PRICE = 223; // See https://legacy.ethgasstation.info/calculatorTxV.php
const ETH_PRICE_USD = 3388;

async function compute_gas_and_price(fun_to_evaluate, ...args) {
  const tx = await fun_to_evaluate(...args, { gasLimit: 30_000_000 });
  const txReceipt = await tx.wait();

  const gasUsed = txReceipt.gasUsed.toString();
  const price = gasUsed * GAS_PRICE * 10 ** -9 * ETH_PRICE_USD;
  return [gasUsed, price];
}

async function print_report(title, fun_to_eval, fun_names, ...args) {
  console.log("**** " + title + "****");
  for (let i = 0; i < fun_to_eval.length; i++) {
    let res;
    try {
      res = await compute_gas_and_price(fun_to_eval[i], ...args);
      let gas = res[0] / N_AAPTX;
      let price = res[1] / N_AAPTX;
      console.log(
        `${fun_names[i]}:  ${gas} gas  ------ ${price.toFixed(1)} USD`
      );
    } catch (error) {
      console.log(error);
    }
  }
  console.log("\n");
}

async function main() {
  let fun_to_eval, fun_names;

  const DPV = await ethers.getContractFactory("DummyValidator");
  const dpv = await DPV.deploy();

  // Polling interval in ms.
  dpv.provider.pollingInterval = 20;

  await dpv.deployed();

  console.log(`Contract deployed at address ${dpv.address}`);

  fun_names = ["verify_empty", "verify", "batch_verify"];
  fun_to_eval = fun_names.map((name) => dpv[name]);

  const chunk = common.create_chunk(N_AAPTX);

  await print_report(
    "NO Merkle tree update",
    fun_to_eval,
    fun_names,
    chunk,
    false,
    false
  );

  await print_report(
    "Merkle tree update (Starkware)",
    fun_to_eval,
    fun_names,
    chunk,
    true,
    true
  );

  await print_report(
    "Merkle tree update (NO Starkware)",
    fun_to_eval,
    fun_names,
    chunk,
    true,
    false
  );

  const Contract = await ethers.getContractFactory("NullifiersMerkleTree");
  const contract = await Contract.deploy();
  contract.provider.pollingInterval = 20;

  await contract.deployed();

  console.log(`Contract deployed at address ${contract.address}`);

  // TODO uncomment

  // const randHash = () =>
  //   Array(4)
  //     .fill()
  //     .map((_) => BigNumber.from(utils.randomBytes(4)));
  //
  // fun_names = ["elem_hash", "leaf_hash"];
  // fun_to_eval = fun_names.map((name) => contract[name]);
  //
  // const nullifier = utils.randomBytes(32);
  // await print_report("elem_hash, leaf_hash", fun_to_eval, fun_names, nullifier);

  // fun_names = ["branch_hash"];
  // fun_to_eval = fun_names.map((name) => contract[name]);

  // const left = randHash();
  // const right = randHash();
  // await print_report("Blake2", fun_to_eval, fun_names, left, right);

  // fun_names = ["terminalNodeValueNonEmpty"];
  // fun_to_eval = fun_names.map((name) => contract[name]);
  // const height = 10;
  // const node = {
  //   isEmptySubtree: false,
  //   height,
  //   elem: nullifier,
  // };
  // await print_report(`Blake2 (height=${height})`, fun_to_eval, fun_names, node);
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
