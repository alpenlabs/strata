from web3 import Web3

TRANSFER_GAS = 100_000
TRANSACTION_TIMEOUT = 30


def make_token_transfer(web3: Web3, amount: int, beneficiary: str) -> dict:
    """
    Performs a token transfer transaction and waits for its receipt.

    :param web3: Web3 instance.
    :param amount: Amount to transfer in wei.
    :param beneficiary: Recipient address as a hex string.
    :return: Transaction receipt dictionary.
    """
    source = web3.address
    dest = web3.to_checksum_address(beneficiary)

    tx_params = {
        "to": dest,
        "value": hex(amount),
        "gas": hex(TRANSFER_GAS),
        "from": source,
    }
    txid = web3.eth.send_transaction(tx_params)
    tx_receipt = web3.eth.wait_for_transaction_receipt(txid, timeout=TRANSACTION_TIMEOUT)
    if tx_receipt.get("status") != 1:
        raise Exception(f"Token transfer transaction '{txid}' failed")

    return tx_receipt
