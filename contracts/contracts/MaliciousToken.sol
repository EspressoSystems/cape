// SPDX-License-Identifier: GPL-3.0-or-later

// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pragma solidity ^0.8.0;

import {TestCAPE} from "./mocks/TestCAPE.sol";

// Learn more about the ERC20 implementation
// on OpenZeppelin docs: https://docs.openzeppelin.com/contracts/4.x/erc20
import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract MaliciousToken is ERC20 {
    address private _targetContractAddress;
    bool private _runDeposit;
    bool private _runSubmitBlock;

    /// @notice MaliciousToken contract constructor.
    constructor() ERC20("Malicious Token", "MAT") {
        _runDeposit = false;
        _runSubmitBlock = false;
    }

    /**
     * /// @dev Sets the address for performing the reentrancy attack.
     */
    function setTargetContractAddress(address targetContractAddress) public {
        _targetContractAddress = targetContractAddress;
    }

    /**
     * /// @dev pick the depositErc20 function when calling back the CAPE contract
     */
    function selectDepositAttack() public {
        _runDeposit = true;
        _runSubmitBlock = false;
    }

    /**
     * /// @dev pick the submitBlock function when calling back the CAPE contract
     */
    function selectSubmitBlockAttack() public {
        _runDeposit = false;
        _runSubmitBlock = true;
    }

    /**
     * /// @notice Malicious implementation of transferFrom
     */
    function transferFrom(
        address,
        address,
        uint256
    ) public virtual override returns (bool) {
        TestCAPE cape = TestCAPE(_targetContractAddress);

        if (_runDeposit) {
            TestCAPE.RecordOpening memory dummyRo;
            address dummyAddress;
            cape.depositErc20(dummyRo, dummyAddress);
        }

        if (_runSubmitBlock) {
            TestCAPE.CapeBlock memory dummyBlock;
            cape.submitCapeBlock(dummyBlock);
        }

        return true;
    }
}
