//SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

import {Queue} from "../Queue.sol";

contract TestQueue is Queue {
    function isQueueEmpty() public returns (bool) {
        return _isQueueEmpty();
    }

    function getQueueSize() public returns (uint256) {
        return _getQueueSize();
    }

    function pushToQueue(uint256 recordCommitment) public {
        _pushToQueue(recordCommitment);
    }

    function getQueueElem(uint256 index) public returns (uint256) {
        return _getQueueElem(index);
    }

    function emptyQueue() public {
        _emptyQueue();
    }
}
