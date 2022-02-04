# Smart contracts


## CAPE Contract

The CAPE Smart contract allows bidirectional transfers of assets between Ethereum and the AAP Blockchain. 
Its design is inspired by Tornado Cash where an ERC20 token transfer can trigger automatically the creation of some asset record inside the AAP Blockchain. 
Transferring assets from AAP to Ethereum relies on the idea of burning/destroying the asset record and mint or unlock it on the other side (Ethereum) also in an atomic fashion.

### Building blocks

* Main contract
* Records Merkle tree
* Plonk Verifier

### Sequence Diagram


## ERC20 contracts