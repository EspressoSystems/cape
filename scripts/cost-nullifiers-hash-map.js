const { ethers } = require("hardhat");
const common = require("../lib/common");

async function print_report(owner, title, fun_to_eval, args) {
  console.log("**** " + title + "****");

  let res;
  try {
    res = await common.compute_gas_and_price(owner, fun_to_eval, args);
    let gas = res[0];

    let price = res[1].getValue();
    console.log(gas + " gas  ------ " + price + " USD ");
  } catch (error) {
    console.log(error);
  }

  console.log("\n");
}

async function main() {
  const [owner] = await ethers.getSigners();

  const CAPE = await ethers.getContractFactory("TestCAPE");
  const cape = await CAPE.deploy();

  // Polling interval in ms.
  cape.provider.pollingInterval = 20;

  await cape.deployed();

  console.log("Contract deployed at address " + cape.address);

  const NUM_MAX_NULLIFIERS = 10_000;

  for (let i = 1; i < NUM_MAX_NULLIFIERS; i += 1_000) {
    // Insert i nullifiers into the hash table
    for (let j = 0; j < i; j++) {
      // Insert nullifiers
      let nullifier = ethers.utils.randomBytes(32);
      await cape._insertNullifier(nullifier);
    }

    // Measure how much it costs to check for membership
    let title = "Check for nullifiers. HASHMAP SIZE = " + i + " ";
    let test_nullifier = ethers.utils.randomBytes(32);
    await print_report(owner, title, cape._hasNullifierAlreadyBeenPublished, [
      test_nullifier,
    ]);

    title = "Insert a nullifier. HASHMAP SIZE = " + i + " ";
    test_nullifier = ethers.utils.randomBytes(32);
    await print_report(owner, title, cape._insertNullifier, [test_nullifier]);
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
