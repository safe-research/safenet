import json, subprocess, sys
out = sys.argv[1]
def canon(t):
    ty = t["type"]
    if ty.startswith("tuple"):
        return "(" + ",".join(canon(c) for c in t["components"]) + ")" + ty[len("tuple"):]
    return ty
def topics(name):
    abi = json.load(open(f"{out}/{name}.sol/{name}.json"))["abi"]
    r = {}
    for e in abi:
        if e["type"] == "event":
            sig = e["name"] + "(" + ",".join(canon(i) for i in e["inputs"]) + ")"
            r[subprocess.check_output(["cast", "keccak", sig], text=True).strip()[2:].lower()] = sig
    return r
code = json.load(open(f"{out}/SentinelOracle.sol/SentinelOracle.json"))["deployedBytecode"]["object"].lower()
foreign = {**topics("FROSTCoordinator"), **topics("Consensus")}
own = topics("SentinelOracle")
print("SentinelOracle runtime bytecode length (bytes):", (len(code) - 2) // 2)
print("Coordinator/Consensus topic0 found in SentinelOracle runtime bytecode:", [s for h, s in foreign.items() if h in code] or "none")
print("SentinelOracle own topic0 found in its runtime bytecode:", sum(1 for h in own if h in code), "of", len(own))
