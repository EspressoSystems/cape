#!/usr/bin/env python3

from brownie import accounts, TestRecordsMerkleTree, web3

# run `brownie run gas-report`
def main():
    tree_depth = 20
    mock_contract = TestRecordsMerkleTree.deploy(tree_depth, {"from": accounts[0]})

    initial_root_value = (
        16338819200219295738128869281163133642735762710891814031809540606861827401155
    )
    initial_num_of_leaves = 1
    leaf_value = (
        17101599813294219906421080963940202236242422543188383858545041456174912634953
    )
    flatten_frontier = [leaf_value] + [0] * 60
    elems = [1, 2, 3, 4, 5]

    mock_contract.testSetRootAndNumLeaves(
        initial_root_value, initial_num_of_leaves, {"from": accounts[0]}
    )

    tx = mock_contract.testUpdateRecordsMerkleTree(
        flatten_frontier, elems, {"from": accounts[0]}
    )
    tx.info()
    tx.status
    # FIXME: for unknown reasons, I got this error:
    # RPCRequestError: Encountered a ConnectionError while requesting `debug_traceTransaction`. The local RPC client has likely crashed.
    #
    # @Philippe, I believe your function didn't really pass the test
    tx.call_trace()
