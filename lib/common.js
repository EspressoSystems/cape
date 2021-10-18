const { ethers } = require("hardhat");
var rand = require("random-seed").create();

module.exports = {
  create_chunk: function (n_aap_tx) {
    const aap_bytes_size = 3000;

    const bytes_len = n_aap_tx * aap_bytes_size;

    let chunk = new Uint8Array(bytes_len);

    // Generate random looking chunk as the AAP transaction contain random values
    rand.initState();
    for (let i = 0; i < bytes_len; i++) {
      let v = rand(2 ** 8 - 1);
      chunk[i] = v;
    }

    return chunk;
  },

  deployNullifierMerkleTreeContract: async function () {
    // const Helpers = await ethers.getContractFactory("helpers");
    // const helpers = await Helpers.deploy();
    // const Contract = await ethers.getContractFactory("NullifiersMerkleTree", {
    //   libraries: {
    //     helpers: helpers.address,
    //   },
    // });

    const Contract = await ethers.getContractFactory("NullifiersMerkleTree");

    const contract = await Contract.deploy();
    await contract.deployed();
    return contract;
  },
};
