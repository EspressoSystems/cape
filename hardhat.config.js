require("@nomiclabs/hardhat-waffle");
const { TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD } = require("hardhat/builtin-tasks/task-names");

const Common = require("@ethereumjs/common").default;
const forCustomChain = Common.forCustomChain;
Common.forCustomChain = (...args) => {
  const common = forCustomChain(...args);
  common._eips = [2537];
  return common;
};


// This is a sample Hardhat task. To learn how to create your own go to
// https://hardhat.org/guides/create-task.html
task("accounts", "Prints the list of accounts", async (taskArgs, hre) => {
  const accounts = await hre.ethers.getSigners();

  for (const account of accounts) {
    console.log(account.address);
  }
});

// Use the compiler downloaded with nix if the version matches
// Based on: https://github.com/fvictorio/hardhat-examples/tree/master/custom-solc
subtask(TASK_COMPILE_SOLIDITY_GET_SOLC_BUILD, async (args, hre, runSuper) => {
  if (args.solcVersion === process.env.SOLC_VERSION) {
    const compilerPath = process.env.SOLC_PATH;

    return {
      compilerPath,
      isSolcJs: false, // native solc
      version: args.solcVersion,
      // for extra information in the build-info files, otherwise not important
      longVersion: `${args.solcVersion}-dummy-long-version`
    }
  }

  console.warn("Warning: Using compiler downloaded by hardhat")
  return runSuper(); // Fall back to running the default subtask
})

// You need to export an object to set up your config
// Go to https://hardhat.org/config/ to learn more

/**
 * @type import('hardhat/config').HardhatUserConfig
 */
module.exports = {
  defaultNetwork: "localhost",
  networks: {
    hardhat: {
    },
    local: {
      url: "http://localhost:8545",
    }
  },
  solidity: {
    compilers: [
      {
        version: process.env.SOLC_VERSION,
      },
      {
        version: "0.4.14"
      }
    ],
    settings: {
      optimizer: {
        enabled: true,
        runs: 1000,
      },
    },
  },
  gasPrice: 4700,
  gasLimit: 300000000,
  chainId: 8889,
};
