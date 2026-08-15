#!/usr/bin/env python3
"""Drive `lua-language-server` over stdio and ask for completions at a marked
position, to verify §1's claim that `---@alias` gives completion *inside* a
string literal.

The `--check` mode used by `run.sh` covers diagnostics; completion has no
headless mode, so this speaks LSP directly.
"""

import json
import os
import subprocess
import sys
import threading
import time

ROOT = os.path.dirname(os.path.abspath(__file__))


class Client:
    def __init__(self, root):
        self.p = subprocess.Popen(
            ["lua-language-server", "--stdio", "--logpath", os.path.join(root, ".out-lsp")],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            cwd=root,
        )
        self.id = 0
        self.replies = {}
        self.lock = threading.Condition()
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        f = self.p.stdout
        while True:
            length = None
            while True:
                line = f.readline()
                if not line:
                    return
                line = line.strip()
                if not line:
                    break
                if line.lower().startswith(b"content-length:"):
                    length = int(line.split(b":")[1])
            if length is None:
                continue
            msg = json.loads(f.read(length))
            if "id" in msg and ("result" in msg or "error" in msg):
                with self.lock:
                    self.replies[msg["id"]] = msg
                    self.lock.notify_all()

    def _send(self, obj):
        data = json.dumps(obj).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(data) + data)
        self.p.stdin.flush()

    def notify(self, method, params):
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method, params, timeout=60):
        self.id += 1
        rid = self.id
        self._send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        deadline = time.time() + timeout
        with self.lock:
            while rid not in self.replies:
                if not self.lock.wait(deadline - time.time()):
                    raise TimeoutError(method)
            return self.replies.pop(rid)


# Each probe is (label, source, line, character). The position is *inside* an
# open string literal, which is the interesting case: LuaLS sees a token, not
# an expression.
PROBES = [
    (
        "alias member inside a string literal (array element)",
        'installer {\n\tpages = { "" },\n}\n',
        1, 12,
        {"Welcome", "Directory", "InstFiles", "Finish", "License", "Components"},
    ),
    (
        "alias member inside a string literal (scalar field)",
        'attributes {\n\tname = "x",\n\toutFile = "y",\n\trequestExecutionLevel = "",\n}\n',
        3, 26,
        {"admin", "user", "highest", "none"},
    ),
    (
        "field completion inside a block",
        "attributes {\n\t\n}\n",
        1, 1,
        {"name", "outFile", "unicode", "crcCheck"},
    ),
    (
        "alias member inside a string literal (nested table arg)",
        'section("Main", function()\n\tmessageBox { text = "t", buttons = "" }\n end)\n',
        1, 38,
        {"YESNO", "OK", "OKCANCEL"},
    ),
]


def main():
    c = Client(ROOT)
    c.request(
        "initialize",
        {
            "processId": os.getpid(),
            "rootUri": "file://" + ROOT,
            "capabilities": {
                "textDocument": {
                    "completion": {"completionItem": {"snippetSupport": True}}
                }
            },
        },
    )
    c.notify("initialized", {})
    time.sleep(3)  # let the workspace (and ./meta) load

    failures = 0
    for i, (label, src, line, char, expected) in enumerate(PROBES):
        uri = "file://%s/probe%d.lua" % (ROOT, i)
        c.notify(
            "textDocument/didOpen",
            {"textDocument": {"uri": uri, "languageId": "lua", "version": 1, "text": src}},
        )
        time.sleep(1.5)
        r = c.request(
            "textDocument/completion",
            {
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": char},
                "context": {"triggerKind": 1},
            },
        )
        res = r.get("result") or {}
        items = res.get("items", res if isinstance(res, list) else [])
        labels = {it["label"].strip('"') for it in items}
        got = expected & labels
        ok = len(got) >= max(2, len(expected) // 2)
        print("%-56s %s" % (label, "PASS" if ok else "FAIL"))
        print("      expected any of: %s" % sorted(expected))
        print("      matched:         %s" % sorted(got))
        if not ok:
            print("      all %d labels:   %s" % (len(labels), sorted(labels)[:25]))
            failures += 1
        c.notify("textDocument/didClose", {"textDocument": {"uri": uri}})

    c.p.kill()
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
