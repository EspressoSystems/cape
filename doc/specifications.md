# CAPE specifications

## Limitations of EVM and impact on the design

* **Ethereum's transactions** are limited in size [(128 kB)](https://github.com/ethereum/go-ethereum/blob/067084fedab1d50e224e40e6442a7740fc53611a/core/tx_pool.go#L49-L53)
  * On the other side CAP transactions are about 3kB so this puts a hard limit on the number of CAP tx we can pack into an ethereum tx (~40 CAP tx/ ETH tx max)
  * However we can try to cut the CAP block into smaller chunks and have these chunks (represented by an Ethereum tx) be mined in a single ethereum block.
  * In more details the solutions would work as follows:
    * relayer collects transactions from CAP users and aggregate those in several chunks. Each chunk can contain in an ethereum transaction and the set of chunks is ordered.
    * The relayer will create an ethereum transaction for each chunk and the corresponding information if needed (hopefully the smart contract can update the blockchain state on its own) to update the blockchain state (e.g.: proofs that the merkle tree roots have been updated correctly) and send each of these ethereum transactions to the smart contract.
    * The ethereum transactions follow the same order as the chunks defined above. This order is enforced by the use of nonces.
    The smart contract receives each ethereum transaction and:
      * Verifies each CAP transaction of the chunk (using the Plonk verifier)
      * Checks that each nullifier is unique
      * Updates the CAP blockchain state on its own or stores the new state of the blockchain contained in the ethereum transaction after verifying the proof of state update (also contained in the ethereum transaction).

    * In the case the smart contract cannot update itself the blockchain state, we will have to deal with the following problem: if  two different relayers try to send eth transactions to the smart contract "at the same time" only one relayer will have its eth transactions mined in the block. The other one will have to try again during the next block.


* **Gas cost**
  * As shown experimentally the plonk verifier [consumes a lot of gas](https://gitlab.com/translucence/cap-on-ethereum/cape/-/issues/17#note_679729222). Potential alternatives to overcome this difficulty are
    * Use recursive snarks as in [AZTEC 2.0](https://hackmd.io/@aztec-network/ByzgNxBfd).
    * Use aggregated snarks. However we are still limited by the CAP transaction size that [is still relatively big even without the proof](https://gitlab.com/translucence/cap-on-ethereum/cape/-/issues/18#aggregated-snarks).
    * Use of TEE approaches such as [Intel SGX enclaves](https://github.com/apache/incubator-teaclave-sgx-sdk).
    * Use of EVM compatible platforms that have [better gas cost efficiency](https://gitlab.com/translucence/cap-on-ethereum/cape/-/issues/23#note_686615980).
    * [Optimistic rollup approach such as Arbitrum](https://github.com/OffchainLabs/arbitrum).
