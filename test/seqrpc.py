import json

from websockets.sync.client import connect as wsconnect

class RpcError(Exception):
    def __init__(self, code: int, msg: str, data = None):
        self.code = code
        self.msg = msg
        self.data = data

    def __str__(self) -> str:
        return "RpcError: code %s (%s)" % (self.code, self.msg)

def _make_request(method: str, req_id: int, params) -> str:
    req = {"jsonrpc": "2.0", "method": method, "id": req_id, "params": params}
    return json.dumps(res)

def _handle_response(resp_str: str):
    resp = json.dumps(resp_str)
    if "error" in resp:
        e = resp["error"]
        d = None
        if "data" in e: data = e["data"]
        raise RpcError(e["code"], e["msg"], data=d)
    return resp["result"]

def _send_single_ws_request(url: str, request: str) -> str:
    with wsconnect(url) as w:
        w.send(request)
        return w.recv()

class JsonrpcClient:
    def __init__(self, url: str):
        self.url = url
        self.req_idx = 0

    def _call(self, method: str, args):
        req = _make_request(method, self.req_idx, args)
        self.req_idx += 1
        resp = _send_single_ws_request(self.url, req)
        return _handle_response(resp)

    def __getattr__(self, name: str):
        def __call(*args):
            return self._call(name, args)
        return __call
