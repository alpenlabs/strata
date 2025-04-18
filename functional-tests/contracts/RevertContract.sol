// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract RevertContract {
    uint256 public counter;

    function revertTransaction() external {
        counter++;
        revert("This transaction has been reverted");
    }
}
