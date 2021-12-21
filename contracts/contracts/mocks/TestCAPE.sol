//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "../CAPE.sol";

contract TestCAPE is CAPE {
    function insertNullifier(uint256 nullifier) public {
        return _insertNullifier(nullifier);
    }

    function checkTransfer(TransferNote memory note) public view {
        return _checkTransfer(note);
    }

    function checkBurn(BurnNote memory note) public view {
        return _checkBurn(note);
    }

    function containsBurnPrefix(bytes memory extraProofBoundData)
        public
        view
        returns (bool)
    {
        return _containsBurnPrefix(extraProofBoundData);
    }

    function containsBurnDestination(bytes memory extraProofBoundData)
        public
        view
        returns (bool)
    {
        return _containsBurnDestination(extraProofBoundData);
    }

    function containsBurnRecord(BurnNote memory note)
        public
        view
        returns (bool)
    {
        return _containsBurnRecord(note);
    }

    function deriveRecordCommitment(RecordOpening memory ro)
        public
        view
        returns (uint256)
    {
        return _deriveRecordCommitment(ro);
    }
}
