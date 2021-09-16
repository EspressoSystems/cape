const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Dummy Plonk Verifier", function () {

  it("Should return the compute the gas fee", async function () {

    const [owner] = await ethers.getSigners();

    const DPV = await ethers.getContractFactory("DummyPlonkVerifier");
    const dpv = await DPV.deploy();
    await dpv.deployed();

    for (let n_aap_tx=0;n_aap_tx<41;n_aap_tx+=10) {

      if (n_aap_tx == 0){
        continue;
      }

      let aap_bytes_size = 3000;
      console.log("****************************");
      console.log("n_aap_tx = " + n_aap_tx);
      let bytes_len = n_aap_tx * aap_bytes_size;

      let chunk = new Uint8Array(bytes_len);
      for (let i=0; i< bytes_len; i++){
        chunk[i] = 12;
      }

      // Only call the function with input
      let tx = await dpv.verify_empty(chunk);
      let txReceipt = await tx.wait();

      let gasUsed = txReceipt.cumulativeGasUsed.toString();
      console.log("Simple call: " + gasUsed);

      // Simple plonk verification
      tx = await dpv.verify(chunk);
      txReceipt = await tx.wait();

      let gasUsedSimplePlonk = txReceipt.cumulativeGasUsed.toString();
      console.log("Simple Plonk Verifier: " + gasUsedSimplePlonk);
      // let expectedGasUsed = ethers.BigNumber.from("2601624");
      // expect(gasUsed).equal(expectedGasUsed);

      // Batch plonk verification
      tx = await dpv.batch_verify(chunk);
      txReceipt = await tx.wait();

      let gasUsedBatchPlonk = txReceipt.cumulativeGasUsed.toString();
      console.log("Batched Plonk Verifier: " + gasUsedBatchPlonk);
      // expectedGasUsed = ethers.BigNumber.from("3169046");
      // expect(gasUsed).equal(expectedGasUsed);
      let ratio = gasUsedBatchPlonk / gasUsedSimplePlonk;
      console.log("Batch v/s simple ratio: " +  ratio);
    }

  });
});
