module FROST = struct
  type verification_key = Placeholder.t
  type signing_share = Placeholder.t
  type nonces = Placeholder.t
  type nonces_commitments = Placeholder.t
  type group_commitment = Placeholder.t
  type share_commitment = Placeholder.t
  type signature_share = Placeholder.t
  type lagrange_coefficient = Placeholder.t

  module Identifier : sig
    type t

    val of_int : int -> t
    val to_int : t -> int
    val equal : t -> t -> bool
    val compare : t -> t -> int
  end = struct
    type t = int

    let of_int = Fun.id
    let to_int = Fun.id
    let equal = Int.equal
    let compare = Int.compare
  end

  module KeyGen = struct
    type coefficients = Placeholder.t
    type commitments = Placeholder.t
    type secret_share = Placeholder.t

    let random_coefficients (_threshold : int) : coefficients =
      Placeholder.fn ()

    let commit (_ : coefficients) : commitments = Placeholder.fn ()
    let verify_proof (_ : commitments) : bool = Placeholder.fn ()

    let secret_share (_ : coefficients) (_ : Identifier.t) : secret_share =
      Placeholder.fn ()

    let encrypt_secret_share (_ : coefficients) (_ : commitments)
        (_ : secret_share) : secret_share =
      Placeholder.fn ()

    let decrypt_secret_share (_ : coefficients) (_ : commitments)
        (_ : secret_share) : secret_share =
      Placeholder.fn ()

    let verify_secret_share (_ : commitments) (_ : secret_share) : bool =
      Placeholder.fn ()

    let group_public_key (_ : (Identifier.t * commitments) list) :
        verification_key =
      Placeholder.fn ()

    let signing_share (_ : (Identifier.t * secret_share) list) : signing_share =
      Placeholder.fn ()
  end

  let generate_nonces (_ : signing_share) : nonces = Placeholder.fn ()

  let signature_commitments (_ : Identifier.t)
      (_ : (Identifier.t * nonces_commitments) list) :
      group_commitment * share_commitment =
    Placeholder.fn ()

  let signature_share (_ : Identifier.t) (_ : signing_share) (_ : string)
      (_ : verification_key) (_ : (Identifier.t * nonces_commitments) list) :
      signature_share
      * group_commitment
      * (Identifier.t * share_commitment * lagrange_coefficient) list =
    Placeholder.fn ()

  let encode_nonces (_ : nonces) : Eth.Abi.t list = Placeholder.fn ()

  let encode_nonces_commitments (_ : nonces_commitments) : Eth.Abi.t list =
    Placeholder.fn ()

  let encode_verification_share (_ : Identifier.t) (_ : share_commitment)
      (_ : lagrange_coefficient) (_ : group_commitment) : Eth.Abi.t list =
    Placeholder.fn ()
end

module Keccak256 = struct
  let hash (_ : string) : string = Placeholder.fn ()
end

module MerkleTree = struct
  type root = string
  type proof = string list

  module type LeafType = sig
    type t

    val encode : t -> string
  end

  module Make (Leaf : LeafType) = struct
    type leaf = Leaf.t
    type t = leaf list

    let root (_ : t) : root = Placeholder.fn ()
    let proof (_ : leaf) (_ : t) : proof = Placeholder.fn ()
  end
end
