require("@nomiclabs/hardhat-waffle");

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
    version: "0.7.2",
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
