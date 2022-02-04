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

    /// @dev constructor method for building the queue
    constructor() {
        _queueSize = 0;
    }

    /// @dev push some element to the queue
    /// @dev for security reason, this function must be internal.
    /// @dev an exception is raised if the queue is already full
    /// @param recordCommitment record commitment to be inserted
    function _pushToQueue(uint256 recordCommitment) internal {
        require(_getQueueSize() < MAX_QUEUE_SIZE, "Pending deposits queue is full");
        _queue[_queueSize] = recordCommitment;
        _queueSize += 1;
    }

    /// @dev check if the queue is empty
    /// @return true if the queue is empty, false otherwise
    function _isQueueEmpty() internal returns (bool) {
        return (_queueSize == 0);
    }

    /// @dev obtain the number of elements in the queue
    /// @return size of the queue
    function _getQueueSize() internal returns (uint256) {
        return _queueSize;
    }

    /// @dev obtains an element of the queue at a specific index
    /// @param index index of the element in the queue
    function _getQueueElem(uint256 index) internal view returns (uint256) {
        return _queue[index];
    }

    /// @dev empty the queue
    function _emptyQueue() internal {
        _queueSize = 0;
    }
}
