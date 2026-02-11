# Glossary

## Cryptographic Terms

### Elliptic Curve

| Term | Definition |
|------|------------|
| **secp256k1** | The elliptic curve used by Bitcoin and Ethereum; also used by FROST in Safenet. Equation: $y² = x³ + 7$ |
| **Point** | A coordinate pair $(x, y)$ on the elliptic curve, or the "point at infinity" |
| **Scalar** | A number in the range $[0, N)$ where $N$ is the curve order |
| **Generator (G)** | The base point from which all other points are derived via scalar multiplication |
| **Curve Order (N)** | The number of points in the curve's group (~2²⁵⁶); scalars are computed mod N |
| **Field Prime (P)** | The prime defining the finite field for coordinates: $2^{256} - 2^{32} - 977$ |
| **Point at Infinity** | The identity element for point addition (like 0 for numbers) |
| **Point Addition** | Combining two curve points $P + Q$; group operation for elliptic curve |
| **Scalar Multiplication** | Multiplying a point by a scalar $k \cdot P$; Adding a points to itself $k$ times. |
| **ECDH** | Elliptic Curve Diffie-Hellman; used to encrypt secret shares during DKG by computing shared secret $a \cdot B = b \cdot A$ |

### FROST

| Term | Definition |
|------|------------|
| **Group Public Key ($Y$)** | The collective public key derived during DKG; can verify signatures from any threshold of participants. Formula: $Y = \sum_{i=1}^{n} C_{i,0}$ |
| **Signing Share ($s_i$)** | A participant's portion of the private key; cannot sign alone. Formula: $s_i = \sum_{j=1}^{n} f_j(i)$ |
| **Public Key Share ($Y_i$)** | The public component of a participant's signing share. Formula: $Y_i = s_i \cdot G$ |
| **Identifier** | A non-zero scalar uniquely identifying a participant within a FROST group |
| **Key Generation Commitment ($C_{i,j}$)** | Public values broadcast during DKG; commits a participant to specific secret polynomial |
| **Key Generation Polynomial** | In DKG, each participant's secret polynomial $f_i(x)$ of degree $t-1$ |
| **Secret Share** | Encrypted polynomial evaluation $f_i(j)$ sent to participant $j$ during DKG |
| **Hiding Nonce ($d$)** | First component of FROST's two-nonce scheme; commitment $D = d \cdot G$ |
| **Binding Nonce ($e$)** | Second component of FROST's two-nonce scheme; commitment $E = e \cdot G$ |
| **Binding Factor ($\rho_i$)** | Value derived from public key, message, and commitments; binds nonces to specific context. Formula: $\rho_i = H_1(Y \| H_4(\text{msg}) \| H_5(\langle j \| D_j \| E_j \rangle_j) \| i)$ |
| **Lagrange Coefficient ($\lambda_i$)** | Multiplier for combining shares in interpolation. Formula: $\lambda_i = \prod_{j \neq i} \frac{j}{j-i}$ |
| **Group Commitment (R)** | Combined commitment point from all signing participants. Formula: $R = \sum_i (D_i + \rho_i E_i)$ |
| **Challenge (c)** | Hash of commitment, public key, and message; core of Schnorr signature. Formula: $c = H_2(R \| Y \| \text{msg})$ |
| **Signature Share ($z_i$)** | Partial signature from one participant. Formula: $z_i = d_i + \rho_i e_i + \lambda_i sk_i c$ |
| **Group Response ($z$)** | The aggregate of the signature shares from all participants. Formula: $z = \sum_i z_i$|
| **Schnorr Signature ($R, z$)** | Final signature $(R, z)$ verifiable with $z \cdot G = R + c \cdot Y$ |
| **Hashing Functions ($H_n$)** | Hashing function specified as part of the FROST ciphersuite |

## Contract Specific Terms

### FROST Coordinator

| Term | Definition |
|------|------------|
| **Group** | A set of participants who completed DKG together |
| **Group ID** | Deterministic identifier for a FROST group |
| **Signature ID** | Identifier for a specific signing ceremony |
| **Sequence** | Counter tracking signing ceremonies within a group |
| **Chunk** | A batch of 1024 nonces committed together |
| **Nonce Commitment** | Pre-committing nonces before messages are known |
| **Complaint** | An accusation that a participant sent invalid data |
| **Confirmation** | A participant's declaration that DKG succeeded for them |

### Consensus Contract

| Term | Definition |
|------|------------|
| **Safe Transaction** | A structured Safe smart account transaction format for cross-chain validation |
| **Domain Separator** | EIP-712 value uniquely identifying the signing domain |
| **Message Hash** | The final hash signed by validators |
| **Callback** | Function called when a ceremony completes |
| **Packets** | Pre-image of a message |

## Data Structure Terms

| Term | Definition |
|------|------------|
| **Merkle Tree** | Data structure for efficient inclusion proofs |
| **Merkle Root** | Single hash summarizing all leaves in a Merkle tree |
| **Merkle Proof** | Path proving a leaf is included in a tree |
| **Doubly Linked List** | List where each node points to previous and next |
| **Head** | First element in a list/queue |
| **Tail** | Last element in a list/queue |

## Abbreviations

| Abbreviation | Full Form |
|--------------|-----------|
| DKG | Distributed Key Generation |
| FROST | Flexible Round-Optimized Schnorr Threshold |
| EIP | Ethereum Improvement Proposal |
| ECDH | Elliptic Curve Diffie-Hellman |
| ECDSA | Elliptic Curve Digital Signature Algorithm |
| RFC | Request for Comments (internet standard) |
| POAP | Proof of Participation (Merkle proof in this context) |
| gid | Group ID |
| sid | Signature ID |
| tx | Transaction |
| SEC | Standards for Efficient Cryptography |
| SHA | Secure Hash Algorithm |

## Security Terms

| Term | Definition |
|------|------------|
| **Rogue Key Attack** | Attack where adversary chooses public key to cancel out honest participants' keys |
| **Adaptive Attack** | Attack where adversary chooses input based on observed protocol messages |
| **Wagner's Birthday Attack** | Attack on multi-signature schemes exploiting hash collisions |
| **Nonce Reuse** | Catastrophic error of using same nonce twice, leading to key extraction |
| **Collusion** | When $t$ or more participants cooperate to compromise security |
| **Key Recovery** | Extracting private key from signatures (possible with nonce reuse) |
| **Denial of Service** | Preventing protocol completion by refusing to participate |

## External References (To Be Moved)

| Resource | Link |
|----------|------|
| RFC-9591 (FROST) | https://datatracker.ietf.org/doc/html/rfc9591 |
| FROST Book | https://frost.zfnd.org/frost.html |
| secp256k1 Parameters | https://www.secg.org/sec2-v2.pdf |
