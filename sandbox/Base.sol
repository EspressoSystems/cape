/*
  Copyright 2019 StarkWare Industries Ltd.

  Licensed under the Apache License, Version 2.0 (the "License").
  You may not use this file except in compliance with the License.
  You may obtain a copy of the License at

  https://www.starkware.co/open-source-license/

  Unless required by applicable law or agreed to in writing,
  software distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions
  and limitations under the License.
*/

pragma solidity ^0.5.2;

contract Base {
    event LogString(string str);

    address payable internal operator;
    uint256 internal constant MINIMUM_TIME_TO_REVEAL = 1 days;
    uint256 internal constant TIME_TO_ALLOW_REVOKE = 7 days;
    bool internal isRevokeStarted = false;
    uint256 internal revokeTime = 0; // The time from which we can revoke.
    bool internal active = true;

    // mapping: (address, commitment) -> time
    // Times from which the users may claim the reward.
    mapping(address => mapping(bytes32 => uint256)) private reveal_timestamps;

    constructor() internal {
        operator = msg.sender;
    }

    modifier onlyOperator() {
        require(msg.sender == operator, "ONLY_OPERATOR");
        _; // The _; defines where the called function is executed.
    }

    function register(bytes32 commitment) public {
        require(
            reveal_timestamps[msg.sender][commitment] == 0,
            "Entry already registered."
        );
        reveal_timestamps[msg.sender][commitment] =
            now +
            MINIMUM_TIME_TO_REVEAL;
    }

    /*
      Makes sure that the commitment was registered at least MINIMUM_TIME_TO_REVEAL before
      the current time.
    */
    function verifyTimelyRegistration(bytes32 commitment) internal view {
        uint256 registrationMaturationTime = reveal_timestamps[msg.sender][
            commitment
        ];
        require(
            registrationMaturationTime != 0,
            "Commitment is not registered."
        );
        require(
            now >= registrationMaturationTime,
            "Time for reveal has not passed yet."
        );
    }

    /*
      WARNING: This function should only be used with call() and not transact().
      Creating a transaction that invokes this function might reveal the collision and make it
      subject to front-running.
    */
    function calcCommitment(
        uint256[] memory firstInput,
        uint256[] memory secondInput
    ) public view returns (bytes32 commitment) {
        address sender = msg.sender;
        uint256 firstLength = firstInput.length;
        uint256 secondLength = secondInput.length;
        uint256[] memory hash_elements = new uint256[](
            1 + firstLength + secondLength
        );
        hash_elements[0] = uint256(sender);
        uint256 offset = 1;
        for (uint256 i = 0; i < firstLength; i++) {
            hash_elements[offset + i] = firstInput[i];
        }
        offset = 1 + firstLength;
        for (uint256 i = 0; i < secondLength; i++) {
            hash_elements[offset + i] = secondInput[i];
        }
        commitment = keccak256(abi.encodePacked(hash_elements));
    }

    function claimReward(
        uint256[] memory firstInput,
        uint256[] memory secondInput,
        string memory solutionDescription,
        string memory name
    ) public {
        require(
            active == true,
            "This challenge is no longer active. Thank you for participating."
        );
        require(firstInput.length > 0, "First input cannot be empty.");
        require(secondInput.length > 0, "Second input cannot be empty.");
        require(
            firstInput.length == secondInput.length,
            "Input lengths are not equal."
        );
        uint256 inputLength = firstInput.length;
        bool sameInput = true;
        for (uint256 i = 0; i < inputLength; i++) {
            if (firstInput[i] != secondInput[i]) {
                sameInput = false;
            }
        }
        require(sameInput == false, "Inputs are equal.");
        bool sameHash = true;
        uint256[] memory firstHash = applyHash(firstInput);
        uint256[] memory secondHash = applyHash(secondInput);
        require(
            firstHash.length == secondHash.length,
            "Output lengths are not equal."
        );
        uint256 outputLength = firstHash.length;
        for (uint256 i = 0; i < outputLength; i++) {
            if (firstHash[i] != secondHash[i]) {
                sameHash = false;
            }
        }
        require(sameHash == true, "Not a collision.");
        verifyTimelyRegistration(calcCommitment(firstInput, secondInput));

        active = false;
        emit LogString(solutionDescription);
        emit LogString(name);
        msg.sender.transfer(address(this).balance);
    }

    function applyHash(uint256[] memory elements)
        public
        view
        returns (uint256[] memory elementsHash)
    {
        elementsHash = sponge(elements);
    }

    function startRevoke() public onlyOperator {
        require(isRevokeStarted == false, "Revoke already started.");
        isRevokeStarted = true;
        revokeTime = now + TIME_TO_ALLOW_REVOKE;
    }

    function revokeReward() public onlyOperator {
        require(isRevokeStarted == true, "Revoke not started yet.");
        require(now >= revokeTime, "Revoke time not passed.");
        active = false;
        operator.transfer(address(this).balance);
    }

    function sponge(uint256[] memory inputs)
        internal
        view
        returns (uint256[] memory outputElements);

    function getStatus() public view returns (bool[] memory status) {
        status = new bool[](2);
        status[0] = isRevokeStarted;
        status[1] = active;
    }
}
