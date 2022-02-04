//SPDX-License-Identifier: Unlicensed
pragma solidity ^0.8.0;

contract Queue {
    mapping(uint256 => uint256) internal _queue;
    uint256 internal _queueSize;

    // In order to avoid the contract running out of gas if the queue is too large
    // we set the maximum number of pending deposits record commitments to process
    // when a new block is submitted. This is a temporary solution.
    // See https://github.com/SpectrumXYZ/cape/issues/400
    uint64 public constant MAX_QUEUE_SIZE = 10;

    constructor() {
        _queueSize = 0;
    }

    // These functions must be internal
    function _pushToQueue(uint256 recordCommitment) internal {
        require(_getQueueSize() < MAX_QUEUE_SIZE, "Pending deposits queue is full");
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
