import time

def wait_with_rpc_health_check(healthcheck_fn, timeout=5):
    """
    Healthcheck for an rpc service based on a function and timeout.
    This function waits until healthcheck passes at the interval of 1 sec
    """
    for _ in range(timeout):
        try:
            healthcheck_fn()
            return True
        except Exception as _:
            print("Still waiting for service")
        time.sleep(1)
    raise TimeoutError
