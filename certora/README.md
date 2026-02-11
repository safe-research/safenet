# Formal Verification

This subdirectory contains formal verification specifications for the Safenet contracts.

## Installation

From the root of the repository.

```sh
python3 -m venv venv
source venv/bin/activate
pip install -r certora/requirements.txt
```

## Verifying Specs

From within a `venv` activated shell (`source venv/bin/activate` from above):

```sh
CERTORAKEY=... certoraRun certora/conf/*.conf
```
