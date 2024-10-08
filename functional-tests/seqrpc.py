import json

from websockets.sync.client import connect as wsconnect


class RpcError(Exception):
    def __init__(self, code: int, msg: str, data=None):
        self.code = code
        self.msg = msg
        self.data = data

    def __str__(self) -> str:
        return f"RpcError: code {self.code} ({self.msg})"


def _make_request(method: str, req_id: int, params) -> str:
    """Assembles a request body from parts."""
    req = {"jsonrpc": "2.0", "method": method, "id": req_id, "params": params}
    return json.dumps(req)


def _handle_response(resp_str: str):
    """Takes a response body and extracts the result or raises the error."""
    resp = json.loads(resp_str)
    if "error" in resp:
        e = resp["error"]
        d = None
        if "data" in e:
            d = e["data"]
        raise RpcError(e["code"], e["message"], data=d)
    return resp["result"]


def _send_single_ws_request(url: str, request: str) -> str:
    with wsconnect(url) as w:
        w.send(request)
        return w.recv()


class JsonrpcClient:
    def __init__(self, url: str):
        self.url = url
        self.req_idx = 0
        # Hook that lets us add a check that runs before every call.
        self._pre_call_hook = None

    def _do_pre_call_check(m: str):
        """Calls the pre-call hook if set."""
        if self._pre_call_hook is not None:
            h = self._pre_call_hook
            r = h(m)
            if type(r) is bool:
                if r == False:
                    raise RuntimeError(f"failed precheck on call to '{m}'")

    def _call(self, method: str, args):
        self._do_pre_call_check(method)
        req = _make_request(method, self.req_idx, args)
        self.req_idx += 1
        resp = _send_single_ws_request(self.url, req)
        return _handle_response(resp)

    def __getattr__(self, name: str):
        def __call(*args):
            return self._call(name, args)

        return __call
