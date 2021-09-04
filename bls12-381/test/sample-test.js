const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("Greeter", function () {

  it("Should return the new greeting once it's changed", async function () {

    const [owner] = await ethers.getSigners();

    const Greeter = await ethers.getContractFactory("Greeter");
    const greeter = await Greeter.deploy("Hello, world!");
    await greeter.deployed();

    expect(await greeter.greet()).to.equal("Hello, world!");

    const setGreetingTx = await greeter.setGreeting("Hola, mundo!");

    // wait until the transaction is mined
    await setGreetingTx.wait();

    let greeting_str = await greeter.greet();
    expect(greeting_str).to.equal("Hola, mundo!");

  });

  it("Should add two points in G1 correctly", async function () {

    const [owner] = await ethers.getSigners();

    const Greeter = await ethers.getContractFactory("Greeter");
    const greeter = await Greeter.deploy("Hello, world!");
    await greeter.deployed();

    // Call BLS12-381 functions
    let point = await greeter.addG1();
    console.log(point);


  });

});
