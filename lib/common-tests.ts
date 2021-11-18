import { BigNumber } from "ethers";

const { ethers } = require("hardhat");

function hashFrontier(flattened_frontier: BigNumber[], uid: BigNumber): BigNumber {
  let l = flattened_frontier.length;
  let abiInputs = [];
  abiInputs.push(uid);

  for (let i = 0; i < l; i++) {
    abiInputs.push(flattened_frontier[i]);
  }

  let inputAbiEncoded = ethers.utils.defaultAbiCoder.encode(["uint256[]"], [abiInputs]);

  let value = ethers.utils.keccak256(inputAbiEncoded);

  return value;
}

export { hashFrontier };
