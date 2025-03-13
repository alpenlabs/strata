// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract OpenDestroy {
    // Allow the contract to receive Ether
    receive() external payable {}

    // Anyone can call this function to self-destruct the contract.
    // All Ether in the contract will be sent to the caller.
    function destroy() public {
        selfdestruct(payable(msg.sender));
    }
}
