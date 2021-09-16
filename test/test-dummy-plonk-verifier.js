const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Dummy Plonk Verifier", function () {

  it("Should return the compute the gas fee", async function () {

    const [owner] = await ethers.getSigners();

    const DPV = await ethers.getContractFactory("DummyPlonkVerifier");
    const dpv = await DPV.deploy();
    await dpv.deployed();

    // TODO increment the AAP block size
    let aap_bytes_size = 3000;
    let n_aap_tx = 40;
    let bytes_len = n_aap_tx * aap_bytes_size;

    let chunk = new Uint8Array(bytes_len);
    for (let i=0; i< bytes_len; i++){
      chunk[i] = 12;
    }

    // Simple plonk verification
    let tx = await dpv.verify(chunk);
    let txReceipt = await tx.wait();

    let gasUsed = txReceipt.cumulativeGasUsed.toString();
    let expectedGasUsed = ethers.BigNumber.from("5235314");
    expect(gasUsed).equal(expectedGasUsed);

    // Batch plonk verification
    tx = await dpv.batch_verify(chunk);
    txReceipt = await tx.wait();

    gasUsed = txReceipt.cumulativeGasUsed.toString();
    expectedGasUsed = ethers.BigNumber.from("1314580");
    expect(gasUsed).equal(expectedGasUsed);

  });
});
