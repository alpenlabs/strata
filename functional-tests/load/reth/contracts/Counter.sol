// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract Counter {
    uint256 private count;

    event CounterIncremented(uint256 newValue);
    event CounterDecremented(uint256 newValue);

    function increment() public {
        count += 1;
        emit CounterIncremented(count);
    }

    function decrement() public {
        require(count > 0, "Counter cannot be negative");
        count -= 1;
        emit CounterDecremented(count);
    }

    function getCount() public view returns (uint256) {
        return count;
    }
}