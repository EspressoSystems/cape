//SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

contract Queue {
    mapping(uint256 => uint256) internal _queue;
    uint256 internal _queueSize;

    constructor() {
        _queueSize = 0;
    }

    // These functions must be internal
    function _pushToQueue(uint256 recordCommitment) internal {
        _queue[_queueSize] = recordCommitment;
        _queueSize += 1;
    }

    function _isQueueEmpty() internal returns (bool) {
        return (_queueSize == 0);
    }

    function _getQueueSize() internal returns (uint256) {
        return _queueSize;
    }

    function _getQueueElem(uint256 index) internal view returns (uint256) {
        return _queue[index];
    }

    function _emptyQueue() internal {
        _queueSize = 0;
    }
}
