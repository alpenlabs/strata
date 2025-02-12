// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// ERC20 standard interface.
interface IERC20 {
    // Field views.
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function allowance(
        address owner,
        address spender
    ) external view returns (uint256);

    // Functions.
    function transfer(
        address recipient,
        uint256 amount
    ) external returns (bool);
    function transferFrom(
        address sender,
        address recipient,
        uint256 amount
    ) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
    function mint(address to, uint256 amount) external;

    // Events.
    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(
        address indexed owner,
        address indexed spender,
        uint256 value
    );
}

// Uniswap poor-man's impl of Pair Contract.
// Has no logic of ratio of token calculations according to the liquidity curve.
// Instead, tokenA and tokenB inside the liquidity pair are always equally valuable,
// meaning that amount of tokenOut is always the same as amount of tokenIn in the swap call.
contract UniswapPair {
    address public tokenA;
    address public tokenB;
    uint256 public reserveA;
    uint256 public reserveB;

    constructor(address _tokenA, address _tokenB) {
        tokenA = _tokenA;
        tokenB = _tokenB;
    }

    function addLiquidity(uint256 amountA, uint256 amountB) external {
        reserveA += amountA;
        reserveB += amountB;
    }

    function swap(uint256 amount, address tokenIn) external {
        require(tokenIn == tokenA || tokenIn == tokenB, "Invalid token");

        address tokenOut = tokenIn == tokenA ? tokenB : tokenA;
        IERC20 tokenInInstance = IERC20(tokenIn);
        IERC20 tokenOutInstance = IERC20(tokenOut);

        require(
            tokenInInstance.transferFrom(msg.sender, address(this), amount),
            "Transfer failed"
        );
        require(
            tokenOutInstance.transfer(msg.sender, amount),
            "Swap transfer failed"
        );

        if (tokenIn == tokenA) {
            reserveA += amount;
            reserveB -= amount;
        } else {
            reserveA -= amount;
            reserveB += amount;
        }
    }
}

// Uniswap Factory Contract. A thin layer to spawn UniswapPairs.
contract UniswapFactory {
    mapping(address => mapping(address => address)) public getPair;
    event PairCreated(
        address indexed token0,
        address indexed token1,
        address pair
    );

    function createPair(
        address tokenA,
        address tokenB
    ) external returns (address pair) {
        require(tokenA != tokenB, "Identical addresses");
        require(getPair[tokenA][tokenB] == address(0), "Pair already exists");

        pair = address(new UniswapPair(tokenA, tokenB));
        getPair[tokenA][tokenB] = pair;
        getPair[tokenB][tokenA] = pair;

        emit PairCreated(tokenA, tokenB, pair);
    }
}

// Uniswap poor-man's Router Contract.
//
// Differences in comparison to the Uniswap on ETH mainnet:
// 1. Has no logic of ratio of token calculations according to the liquidity curve.
// Instead, tokenA and tokenB inside the liquidity pair are always equally valuable,
// meaning that amount of tokenOut is always the same as amount of tokenIn in the swap call.
// 2. All swaps are fee-free and liquidity providers do not receive any rewards.
contract UniswapRouter {
    address public factory;

    constructor(address _factory) {
        factory = _factory;
    }

    function addLiquidity(
        address tokenA,
        address tokenB,
        uint256 amountA,
        uint256 amountB
    ) external {
        address pair = UniswapFactory(factory).createPair(tokenA, tokenB);
        require(pair != address(0), "Pair does not exist");

        // Take tokens from the caller.
        IERC20(tokenA).transferFrom(msg.sender, pair, amountA);
        IERC20(tokenB).transferFrom(msg.sender, pair, amountB);

        // And deposit them to the liquidity pair.
        UniswapPair(pair).addLiquidity(amountA, amountB);
    }

    function swap(address tokenIn, address tokenOut, uint256 amount) external {
        address pair = UniswapFactory(factory).getPair(tokenIn, tokenOut);
        require(pair != address(0), "Pair does not exist");

        IERC20 tokenInInstance = IERC20(tokenIn);
        require(
            tokenInInstance.transferFrom(msg.sender, address(this), amount),
            "Token transfer failed"
        );
        tokenInInstance.approve(pair, amount);
        //
        UniswapPair(pair).swap(amount, tokenIn);
        //
        IERC20 tokenOutInstance = IERC20(tokenOut);
        tokenOutInstance.transfer(msg.sender, amount);
    }
}
