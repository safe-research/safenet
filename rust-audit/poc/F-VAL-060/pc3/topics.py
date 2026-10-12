import json, subprocess, sys
out = sys.argv[1]
def canon(t):
    ty = t["type"]
    if ty.startswith("tuple"):
        inner = ",".join(canon(c) for c in t["components"])
        return "(" + inner + ")" + ty[len("tuple"):]
    return ty
def events(name):
    abi = json.load(open(f"{out}/{name}.sol/{name}.json"))["abi"]
    res = {}
    for e in abi:
        if e["type"] == "event":
            sig = e["name"] + "(" + ",".join(canon(i) for i in e["inputs"]) + ")"
            h = subprocess.check_output(["cast", "keccak", sig], text=True).strip()
            res[h] = sig
    return res
so = events("SentinelOracle")
co = events("FROSTCoordinator")
cs = events("Consensus")
print("SentinelOracle events:", len(so))
for h, s in sorted(so.items(), key=lambda x: x[1]): print("  ", h, s)
print("FROSTCoordinator events:", len(co), " Consensus events:", len(cs))
overlap = (set(so) & set(co)) | (set(so) & set(cs))
print("overlap of SentinelOracle topic0 with Coordinator or Consensus topic0:", sorted(overlap) or "none")
