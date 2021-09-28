const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Dummy Plonk Verifier", function () {
  it("Should return the compute the gas fee", async function () {
    const [owner] = await ethers.getSigners();

    const DPV = await ethers.getContractFactory("DummyPlonkVerifier");
    const dpv = await DPV.deploy();
    await dpv.deployed();

    let n_aap_tx = 10;

    let aap_bytes_size = 3000;

    let bytes_len = n_aap_tx * aap_bytes_size;

    let chunk = new Uint8Array(bytes_len);
    for (let i = 0; i < bytes_len; i++) {
      chunk[i] = 12;
    }

    // Only call the function with input
    let tx = await dpv.verify_empty(chunk);
    let txReceipt = await tx.wait();

    let gasUsed = txReceipt.cumulativeGasUsed.toString();
    let expectedGasUsed = ethers.BigNumber.from("509343");
    expect(gasUsed).equal(expectedGasUsed);

    // Simple plonk verification
    tx = await dpv.verify(chunk);
    txReceipt = await tx.wait();

    let gasUsedSimplePlonk = txReceipt.cumulativeGasUsed.toString();
    expectedGasUsed = ethers.BigNumber.from("3767429");
    expect(gasUsedSimplePlonk).equal(expectedGasUsed);

    // Batch plonk verification
    tx = await dpv.batch_verify(chunk);
    txReceipt = await tx.wait();

    let gasUsedBatchPlonk = txReceipt.cumulativeGasUsed.toString();
    expectedGasUsed = ethers.BigNumber.from("3165011");
    expect(gasUsedBatchPlonk).equal(expectedGasUsed);

    let ratio = gasUsedBatchPlonk / gasUsedSimplePlonk;
    expect(ratio).lt(1);
  });
});
