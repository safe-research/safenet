module IntMap = Map.Make (Int)
module StringMap = Map.Make (String)

module Address = struct
  type t = unit (* place holder *)

  let equal (_ : t) (_ : t) : bool = failwith "place holder"
end

module Secp256k1 = struct
  type secret_key = unit (* place holder *)
  type public_key = unit (* place holder *)
  type scalar = unit (* place holder *)
  type point = unit (* place holder *)

  let infinity : point = () (* place holder *)
  let random_scalar () : scalar = failwith "place holder"
  let g (_ : scalar) : point = failwith "place holder"
  let add (_ : point) (_ : point) : point = failwith "place holder"
  let addr (_ : secret_key) : Address.t = failwith "place holder"
  let equal (_ : public_key) (_ : public_key) : bool = failwith "place holder"
end

module FROST = struct
  type key_gen_commitment = {
    c : Secp256k1.point list;
    r : Secp256k1.point;
    mu : Secp256k1.scalar;
  }

  type key_gen_secret_shares = {
    y : Secp256k1.point;
    f : Secp256k1.scalar list;
  }

  type nonces = { hiding : Secp256k1.scalar; binding : Secp256k1.scalar }

  type nonces_commitments = {
    hiding : Secp256k1.point;
    binding : Secp256k1.point;
  }

  let key_gen_commit (_ : Secp256k1.scalar list) : key_gen_commitment =
    failwith "place holder"

  let key_gen_verify_proof (_ : key_gen_commitment) : bool =
    failwith "place holder"

  let key_gen_group_public_key (_ : Secp256k1.point list IntMap.t) :
      Secp256k1.point =
    failwith "place holder"

  let key_gen_secret_shares (_ : int) (_ : Secp256k1.scalar list)
      (_ : Secp256k1.point list IntMap.t) :
      Secp256k1.scalar * key_gen_secret_shares =
    failwith "place holder"

  let key_gen_get_secret_share (_ : int) (_ : int)
      (_ : Secp256k1.point list IntMap.t) (_ : key_gen_secret_shares) :
      Secp256k1.scalar option =
    failwith "place holder"

  let key_gen_participant_secret (_ : int) (_ : Secp256k1.scalar IntMap.t) :
      Secp256k1.secret_key =
    failwith "place holder"

  let preprocess_generate_nonce (_ : Secp256k1.secret_key) : nonces =
    failwith "place holder"

  let signature_commitments (_ : int) (_ : nonces_commitments IntMap.t) :
      Secp256k1.point * Secp256k1.point =
    failwith "place holder"

  let signature_share (_ : Secp256k1.secret_key) (_ : int) (_ : string)
      (_ : nonces_commitments IntMap.t) : Secp256k1.scalar * string =
    failwith "place holder"
end

module Abi = struct
  let encode (_ : 'a list) (_ : 'b) : string = failwith "place holder"
  let encode_packed (_ : 'a list) (_ : 'b) : string = failwith "place holder"

  let encode_with_selector (_ : 'a) (_ : 'b list) (_ : 'c) : string =
    failwith "place holder"

  let hash_typed_data (_ : 'a) : string = failwith "place holder"
end

module MerkleTree = struct
  let root (_ : 'a) : string = failwith "place holder"
  let proof (_ : 'a) (_ : 'b) : string list = failwith "place holder"
end

module ParticipantSet = struct
  type t = unit (* place holder *)

  let init (_ : Address.t list) : t = failwith "place holder"
  let remove (_ : t) (_ : 'a) : t = failwith "place holder"
  let remove_missing (_ : t) (_ : 'a) : t = failwith "place holder"
  let cardinal (_ : t) : int = failwith "place holder"
  let identifier (_ : t) (_ : Address.t) : int = failwith "place holder"
  let addr (_ : t) (_ : int) : Address.t = failwith "place holder"
end

type group_id = string

module FROSTCoordinator = struct
  let group_id (_ : ParticipantSet.t) (_ : int) (_ : int) (_ : string) :
      group_id =
    failwith "place holder"
end

type group = {
  id : group_id;  (** The set of participants in the group. *)
  participants : ParticipantSet.t;
      (** The identifier of the validator in the group. *)
  key : Secp256k1.public_key;  (** The participant share ID. *)
  share : Secp256k1.secret_key;
}

type epoch = { id : int; group : group }

type rollover =
  | Collecting_key_gen_commitments of {
      epoch : int;
      id : group_id;
      coefficients : Secp256k1.scalar list;
      commitments : Secp256k1.point list IntMap.t;
      participants : ParticipantSet.t;
      deadline : int;
    }
  | Collecting_key_gen_secret_shares of {
      epoch : int;
      id : group_id;
      key : Secp256k1.point;
      commitments : Secp256k1.point list IntMap.t;
      shares : Secp256k1.scalar IntMap.t;
      last_participant : Address.t;
      participants : ParticipantSet.t;
      deadline : int;
    }
  | Signing_epoch_rollover of { epoch : epoch; message : string }
  | Staged_epoch of epoch

type preprocess_group = {
  nonces : FROST.nonces IntMap.t;
  pending : FROST.nonces list StringMap.t;
}

type signature_id = string

type sign_status =
  | Waiting_for_sign_request of { responsible : Address.t }
  | Collecting_sign_nonces of {
      id : signature_id;
      nonces : FROST.nonces_commitments IntMap.t;
    }
  | Collecting_sign_shares of {
      id : signature_id;
      root : string;
      group_commitment : Secp256k1.point;
      shares : Secp256k1.scalar IntMap.t;
    }
  | Waiting_for_consensus_attestation of {
      id : signature_id;
      responsible : Address.t;
    }

type packet =
  | Epoch_rollover of { epoch : int; rollover : int; group_id : group_id }
  | Transaction_proposal of { epoch : int; hash : string }

type signing_ceremony = {
  status : sign_status;
  epoch : epoch;
  packet : packet;
  last_participant : Address.t;
  selection : ParticipantSet.t;
  deadline : int;
}

type state = {
  block : int;
  active_epoch : epoch;
  rollover : rollover;
  preprocess : preprocess_group StringMap.t;
  signing_ceremonies : signing_ceremony StringMap.t;
}
(** The validator state. *)

(** Create a validator parameterized over some configuration. *)
module MakeValidator (Configuration : sig
  val account : Secp256k1.secret_key
  val blocks_per_epoch : int
  val all_participants : Address.t list
  val consensus : Address.t
  val nonces_chunk_size : int
  val min_remaining_nonces : int
  val key_gen_block_timeout : int
  val signing_ceremony_block_timeout : int
end) =
struct
  let () =
    if Configuration.(min_remaining_nonces > nonces_chunk_size) then
      failwith "invalid configuration"

  let assert_assumption c m = if c then () else failwith m
  let account_address = Secp256k1.addr Configuration.account

  let group_parameters participants =
    let count = ParticipantSet.cardinal participants in
    assert_assumption (count >= 2)
      "we always have at least 2 functioning validators";
    let threshold = (count / 2) + 1 in
    (count, threshold)

  let key_gen_context epoch =
    let version = 0 in
    Abi.encode_packed
      [ `Uint32; `Address; `Uint64 ]
      (version, Configuration.consensus, epoch)

  let key_gen_and_commit state participants =
    let epoch = 1 + (state.block / Configuration.blocks_per_epoch) in
    let count, threshold = group_parameters participants in
    let context = key_gen_context epoch in
    let coefficients =
      List.init threshold (fun _ -> Secp256k1.random_scalar ())
    in
    let state' =
      {
        state with
        rollover =
          Collecting_key_gen_commitments
            {
              epoch;
              id =
                FROSTCoordinator.group_id participants count threshold context;
              commitments = IntMap.empty;
              coefficients;
              participants;
              deadline = state.block + Configuration.key_gen_block_timeout;
            };
      }
    in
    let identifier = ParticipantSet.identifier participants account_address in
    let proof = MerkleTree.proof participants account_address in
    let commitments = FROST.key_gen_commit coefficients in
    let actions =
      [
        `Coordinator_key_gen_and_commit
          ( MerkleTree.root participants,
            count,
            threshold,
            context,
            identifier,
            proof,
            commitments );
      ]
    in
    (state', actions)

  let rollover_block epoch = epoch * Configuration.blocks_per_epoch

  let epoch_rollover state =
    let key_gen_and_commit_with_all st =
      (* Always use the full participant set at the start of the epoch, even
         if they were previously misbahaving. In the future, the participant
         selection can be stricter and inspect onchain state of the staking
         contract. *)
      let all_participants =
        Configuration.all_participants |> ParticipantSet.init
      in
      key_gen_and_commit st all_participants
    in
    match state.rollover with
    (* The previous KeyGen ceremony took too long, try again. *)
    | Collecting_key_gen_commitments { epoch; _ }
    | Collecting_key_gen_secret_shares { epoch; _ }
    | Signing_epoch_rollover { epoch = { id = epoch; _ }; _ }
      when rollover_block epoch = state.block ->
        key_gen_and_commit_with_all state
    | Staged_epoch staged when rollover_block staged.id = state.block ->
        let state' = { state with active_epoch = staged } in
        key_gen_and_commit_with_all state'
    | _ -> (state, [])

  let key_gen_wait state =
    match state.rollover with
    | Collecting_key_gen_commitments { participants; deadline; commitments; _ }
      when deadline = state.block ->
        let participants' =
          ParticipantSet.remove_missing participants commitments
        in
        key_gen_and_commit state participants'
    | Collecting_key_gen_secret_shares { participants; deadline; shares; _ }
      when deadline = state.block ->
        let participants' = ParticipantSet.remove_missing participants shares in
        key_gen_and_commit state participants'
    | _ -> (state, [])

  let key_gen_commitment state group_id identifier commitment =
    match state.rollover with
    | Collecting_key_gen_commitments
        { epoch; id; coefficients; commitments; participants; deadline }
      when id == group_id ->
        if FROST.key_gen_verify_proof commitment then
          let commitments' = IntMap.add identifier commitment.c commitments in
          if
            ParticipantSet.cardinal participants == IntMap.cardinal commitments'
          then
            let me = ParticipantSet.identifier participants account_address in
            let self, shares =
              FROST.key_gen_secret_shares me coefficients commitments'
            in
            let key = FROST.key_gen_group_public_key commitments' in
            let rollover' =
              Collecting_key_gen_secret_shares
                {
                  epoch;
                  id;
                  key;
                  commitments;
                  shares = IntMap.(add me self empty);
                  last_participant = ParticipantSet.addr participants identifier;
                  participants;
                  deadline = state.block + Configuration.key_gen_block_timeout;
                }
            in
            ( { state with rollover = rollover' },
              [
                `Coordinator_key_gen_secret_share_with_callback
                  ( id,
                    shares,
                    ( Configuration.consensus,
                      Abi.encode [ `Uint64; `Uint64 ]
                        (epoch, rollover_block epoch) ) );
              ] )
          else
            let rollover' =
              Collecting_key_gen_commitments
                {
                  epoch;
                  id;
                  coefficients;
                  commitments = commitments';
                  participants;
                  deadline;
                }
            in
            ({ state with rollover = rollover' }, [])
        else
          let participants' = ParticipantSet.remove participants identifier in
          key_gen_and_commit state participants'
    | _ -> (state, [])

  let key_gen_secret_shares state group_id identifier secret_shares =
    match state.rollover with
    | Collecting_key_gen_secret_shares
        {
          epoch;
          id;
          key;
          commitments;
          shares;
          last_participant;
          participants;
          deadline;
        }
      when id == group_id -> begin
        let me = ParticipantSet.identifier participants account_address in
        match
          FROST.key_gen_get_secret_share me identifier commitments secret_shares
        with
        | Some my_share ->
            let shares' = IntMap.add identifier my_share shares in
            if ParticipantSet.cardinal participants == IntMap.cardinal shares'
            then
              let share = FROST.key_gen_participant_secret me shares' in
              let epoch =
                {
                  id = epoch;
                  group = { id = group_id; participants; key; share };
                }
              in
              let packet =
                Epoch_rollover
                  {
                    epoch = epoch.id;
                    rollover = rollover_block epoch.id;
                    group_id;
                  }
              in
              let message = Abi.hash_typed_data packet in
              let rollover' = Signing_epoch_rollover { epoch; message } in
              let signing_ceremonies' =
                StringMap.add message
                  {
                    status =
                      Waiting_for_sign_request
                        {
                          responsible =
                            ParticipantSet.addr participants identifier;
                        };
                    epoch = state.active_epoch;
                    packet;
                    last_participant =
                      ParticipantSet.addr participants identifier;
                    selection = state.active_epoch.group.participants;
                    deadline =
                      state.block + Configuration.signing_ceremony_block_timeout;
                  }
                  state.signing_ceremonies
              in

              let state' =
                {
                  state with
                  rollover = rollover';
                  signing_ceremonies = signing_ceremonies';
                }
              in

              (state', [])
            else
              let rollover' =
                Collecting_key_gen_secret_shares
                  {
                    epoch;
                    id;
                    key;
                    commitments;
                    shares = shares';
                    last_participant;
                    participants;
                    deadline;
                  }
              in
              ({ state with rollover = rollover' }, [])
        | None ->
            let participants' = ParticipantSet.remove participants identifier in
            key_gen_and_commit state participants'
      end
    | _ -> (state, [])

  let preprocess state group_id chunk nonces_commitments =
    let state' =
      match StringMap.find_opt group_id state.preprocess with
      | Some preprocess_group -> begin
          match
            StringMap.find_opt nonces_commitments preprocess_group.pending
          with
          | Some nonces ->
              let pending' =
                StringMap.remove nonces_commitments preprocess_group.pending
              in
              let nonces', _ =
                List.fold_left
                  (fun (nonces', sequence) n ->
                    (IntMap.add sequence n nonces', sequence + 1))
                  ( preprocess_group.nonces,
                    chunk * Configuration.nonces_chunk_size )
                  nonces
              in
              let preprocess_group' =
                { pending = pending'; nonces = nonces' }
              in
              let preprocess' =
                StringMap.add group_id preprocess_group' state.preprocess
              in
              { state with preprocess = preprocess' }
          | None -> state
        end
      | None -> state
    in
    (state', [])

  let signing_ceremony_request state group_id message signature_id sequence =
    match StringMap.find_opt message state.signing_ceremonies with
    | Some signing_ceremony when signing_ceremony.epoch.group.id == group_id ->
      begin
        match signing_ceremony.status with
        | Waiting_for_sign_request _ -> begin
            let signing_ceremony' =
              {
                signing_ceremony with
                status =
                  Collecting_sign_nonces
                    { id = signature_id; nonces = IntMap.empty };
                deadline =
                  state.block + Configuration.signing_ceremony_block_timeout;
              }
            in
            let signing_ceremonies' =
              StringMap.add message signing_ceremony' state.signing_ceremonies
            in
            let state' =
              { state with signing_ceremonies = signing_ceremonies' }
            in
            let preprocess_group =
              StringMap.find_opt group_id state.preprocess
            in
            match
              Option.bind preprocess_group (fun { nonces; _ } ->
                  let _, n, rest = IntMap.split sequence nonces in
                  Option.map (fun n -> (n, rest)) n)
            with
            | Some (n, preprocess_group_nonces') ->
                let preprocess_group = Option.get preprocess_group in

                let pending', actions_preprocess =
                  if
                    IntMap.cardinal preprocess_group_nonces'
                    < Configuration.min_remaining_nonces
                    && StringMap.is_empty preprocess_group.pending
                  then
                    let nonces_chunk =
                      List.init Configuration.nonces_chunk_size (fun _ ->
                          FROST.preprocess_generate_nonce
                            signing_ceremony.epoch.group.share)
                    in
                    let nonces_commitment = MerkleTree.root nonces_chunk in
                    let pending' =
                      StringMap.add nonces_commitment nonces_chunk
                        preprocess_group.pending
                    in
                    ( pending',
                      [ `Coordinator_preprocess (group_id, nonces_commitment) ]
                    )
                  else (preprocess_group.pending, [])
                in

                let preprocess_group' =
                  { nonces = preprocess_group_nonces'; pending = pending' }
                in
                let preprocess' =
                  StringMap.add group_id preprocess_group' state'.preprocess
                in
                let state' = { state' with preprocess = preprocess' } in
                let proof = MerkleTree.proof preprocess_group_nonces' in

                ( state',
                  `Coordinator_sign_reveal_nonces (n, proof)
                  :: actions_preprocess )
            | None -> (state', [])
          end
        | _ -> (state, [])
      end
    | _ -> (state, [])

  let sign_share_with_callback signing_ceremony message signature_id nonces =
    let me =
      ParticipantSet.identifier signing_ceremony.selection account_address
    in
    let group_commitment, share_commitment =
      FROST.signature_commitments me nonces
    in
    let share, root =
      FROST.signature_share signing_ceremony.epoch.group.share me message nonces
    in
    let status' =
      Collecting_sign_shares
        { id = signature_id; root; group_commitment; shares = IntMap.empty }
    in
    let proof = MerkleTree.proof root share_commitment in
    let callback_context =
      match signing_ceremony.packet with
      | Epoch_rollover { epoch; rollover; _ } ->
          Abi.encode_with_selector `Consensus_propose_epoch [ `Uint64; `Uint64 ]
            (epoch, rollover)
      | Transaction_proposal { epoch; hash } ->
          Abi.encode_with_selector `Consensus_attest_transaction
            [ `Uint64; `Bytes32 ] (epoch, hash)
    in
    ( status',
      `Coordinator_sign_share_with_callback
        ( signature_id,
          (share_commitment, root),
          share,
          proof,
          (Configuration.consensus, callback_context) ) )

  let signing_ceremony_reveal_nonce state signature_id identifier
      nonces_commitments =
    match
      List.find_map (fun (message, signing_ceremony) ->
          match signing_ceremony.status with
          | Collecting_sign_nonces { id; nonces } when id == signature_id ->
              Some (message, signing_ceremony, nonces)
          | _ -> None)
      @@ StringMap.to_list state.signing_ceremonies
    with
    | Some (message, signing_ceremony, nonces) ->
        let nonces' = IntMap.add identifier nonces_commitments nonces in
        let status', actions =
          if
            IntMap.cardinal nonces'
            = ParticipantSet.cardinal signing_ceremony.selection
          then
            let status', action =
              sign_share_with_callback signing_ceremony message signature_id
                nonces'
            in
            (status', [ action ])
          else
            let status' =
              Collecting_sign_nonces { id = signature_id; nonces = nonces' }
            in
            (status', [])
        in
        let signing_ceremony' =
          {
            signing_ceremony with
            status = status';
            deadline =
              state.block + Configuration.signing_ceremony_block_timeout;
          }
        in
        let signing_ceremonies' =
          StringMap.add message signing_ceremony' state.signing_ceremonies
        in
        let state' = { state with signing_ceremonies = signing_ceremonies' } in
        (state', actions)
    | None -> (state, [])

  let signing_ceremony_share state signature_id identifier signature_share
      binding_root =
    let state' =
      match
        List.find_map (fun (message, signing_ceremony) ->
            match signing_ceremony.status with
            | Collecting_sign_shares { id; root; group_commitment; shares }
              when id == signature_id && root == binding_root ->
                Some (message, signing_ceremony, group_commitment, shares)
            | _ -> None)
        @@ StringMap.to_list state.signing_ceremonies
      with
      | Some (message, signing_ceremony, group_commitment, shares) ->
          let shares' = IntMap.add identifier signature_share shares in
          let status' =
            if
              IntMap.cardinal shares'
              = ParticipantSet.cardinal signing_ceremony.selection
            then
              let status' =
                Waiting_for_consensus_attestation
                  {
                    id = signature_id;
                    responsible =
                      ParticipantSet.addr signing_ceremony.selection identifier;
                  }
              in
              status'
            else
              Collecting_sign_shares
                {
                  id = signature_id;
                  root = binding_root;
                  group_commitment;
                  shares = shares';
                }
          in
          let signing_ceremony' =
            {
              signing_ceremony with
              status = status';
              deadline =
                state.block + Configuration.signing_ceremony_block_timeout;
            }
          in
          let signing_ceremonies' =
            StringMap.add message signing_ceremony' state.signing_ceremonies
          in
          let state' =
            { state with signing_ceremonies = signing_ceremonies' }
          in
          state'
      | None -> state
    in
    (state', [])

  let signing_ceremonies_wait state =
    let signing_ceremonies', actions =
      StringMap.fold
        (fun message signing_ceremony (signing_ceremonies, actions) ->
          let signing_ceremony', actions =
            if signing_ceremony.deadline = state.block then (
              let selection' =
                match signing_ceremony.status with
                | Collecting_sign_nonces { nonces; _ } ->
                    ParticipantSet.remove_missing signing_ceremony.selection
                      nonces
                | Collecting_sign_shares { shares; _ } ->
                    ParticipantSet.remove_missing signing_ceremony.selection
                      shares
                | Waiting_for_sign_request { responsible }
                | Waiting_for_consensus_attestation { responsible; _ } ->
                    ParticipantSet.remove signing_ceremony.selection responsible
              in
              let _, threshold =
                group_parameters signing_ceremony.epoch.group.participants
              in
              assert_assumption
                (ParticipantSet.cardinal selection' >= threshold)
                "we always have an honest majority of participants.";

              let status', action =
                match signing_ceremony.status with
                | Waiting_for_consensus_attestation { id = signature_id; _ } ->
                    (* As an optimization, we just re-attest if we already have
                       a valid signature but the responsible participant did not
                       do it in time. The punishment for doing this is foregoing
                       rewards for that attestation. *)
                    let status' =
                      Waiting_for_consensus_attestation
                        {
                          id = signature_id;
                          responsible = signing_ceremony.last_participant;
                        }
                    in
                    (* The actual attestation action depends on the packet we are
                       attesting to. *)
                    let action =
                      match signing_ceremony.packet with
                      | Epoch_rollover { epoch; rollover; group_id } ->
                          `Consensus_stage_epoch
                            (epoch, rollover, group_id, signature_id)
                      | Transaction_proposal { epoch; hash } ->
                          `Consensus_attest_transaction
                            (epoch, hash, signature_id)
                    in
                    (status', action)
                | Collecting_sign_nonces { id; nonces; _ } ->
                    (* Continue the signing process with only the participants
                       which provided nonces. Note that this is sound given the
                       asserted assumption above - i.e. we always have a
                       threshold of well-behaving signers. *)
                    sign_share_with_callback signing_ceremony message id nonces
                | _ ->
                    ( Waiting_for_sign_request
                        { responsible = signing_ceremony.last_participant },
                      `Coordinator_sign
                        (signing_ceremony.epoch.group.id, message) )
              in
              let signing_ceremony' =
                {
                  signing_ceremony with
                  status = status';
                  last_participant = account_address;
                  selection = selection';
                  deadline =
                    state.block + Configuration.signing_ceremony_block_timeout;
                }
              in
              let actions' =
                if
                  Address.equal signing_ceremony.last_participant
                    account_address
                then action :: actions
                else actions
              in
              (signing_ceremony', actions'))
            else (signing_ceremony, actions)
          in
          let signing_ceremonies' =
            StringMap.add message signing_ceremony' signing_ceremonies
          in
          (signing_ceremonies', actions))
        state.signing_ceremonies (StringMap.empty, [])
    in
    let state' = { state with signing_ceremonies = signing_ceremonies' } in
    (state', actions)

  let epoch_staged state proposed_epoch =
    match state.rollover with
    | Signing_epoch_rollover { epoch; message } when epoch.id == proposed_epoch
      ->
        let rollover' = Staged_epoch epoch in
        let nonces_chunk =
          List.init Configuration.nonces_chunk_size (fun _ ->
              FROST.preprocess_generate_nonce epoch.group.share)
        in
        let nonces_commitment = MerkleTree.root nonces_chunk in
        let preprocess' =
          StringMap.add epoch.group.id
            {
              nonces = IntMap.empty;
              pending =
                StringMap.add nonces_commitment nonces_chunk StringMap.empty;
            }
            state.preprocess
        in
        let signing_ceremonies' =
          StringMap.remove message state.signing_ceremonies
        in
        let state' =
          {
            state with
            rollover = rollover';
            preprocess = preprocess';
            signing_ceremonies = signing_ceremonies';
          }
        in
        (state', [ `Coordinator_preprocess (epoch.group.id, nonces_commitment) ])
    | _ -> (state, [])

  let validate_transaction _ =
    (* TBD. *)
    true

  let transaction_proposed state message transaction_hash epoch transaction =
    if epoch == state.active_epoch.id && validate_transaction transaction then
      let signing_ceremonies' =
        StringMap.add message
          {
            status = Waiting_for_sign_request { responsible = account_address };
            epoch = state.active_epoch;
            packet = Transaction_proposal { epoch; hash = transaction_hash };
            last_participant = account_address;
            selection = state.active_epoch.group.participants;
            deadline =
              state.block + Configuration.signing_ceremony_block_timeout;
          }
          state.signing_ceremonies
      in
      let state' = { state with signing_ceremonies = signing_ceremonies' } in
      (state', [])
    else (state, [])

  let transaction_attested state message =
    let signing_ceremonies' =
      StringMap.remove message state.signing_ceremonies
    in
    ({ state with signing_ceremonies = signing_ceremonies' }, [])

  (** The state transition function for a validator. *)
  let transition state = function
    | `Chain_block block_number ->
        let state' = { state with block = block_number } in
        let state', actions_r = epoch_rollover state' in
        let state', actions_kgw = key_gen_wait state' in
        let state', actions_sw = signing_ceremonies_wait state' in
        (state', actions_r @ actions_kgw @ actions_sw)
    | `Coordinator_key_gen_committed (group_id, identifier, commitment, _) ->
        key_gen_commitment state group_id identifier commitment
    | `Coordinator_key_gen_secret_shared (group_id, identifier, secret_shares, _)
      ->
        key_gen_secret_shares state group_id identifier secret_shares
    | `Coordinator_preprocess (group_id, _, chunk, nonces_commitment) ->
        preprocess state group_id chunk nonces_commitment
    | `Coordinator_sign (_, group_id, message, signature_id, sequence) ->
        signing_ceremony_request state group_id message signature_id sequence
    | `Coordinator_sign_revealed_nonces
        (signature_id, identifier, nonces_commitments) ->
        signing_ceremony_reveal_nonce state signature_id identifier
          nonces_commitments
    | `Coordinator_sign_shared
        (signature_id, identifier, signature_share, binding_root) ->
        signing_ceremony_share state signature_id identifier signature_share
          binding_root
    | `Consensus_epoch_staged (_, proposed_epoch, _, _) ->
        epoch_staged state proposed_epoch
    | `Consensus_transaction_proposed
        (message, transaction_hash, epoch, transaction) ->
        transaction_proposed state message transaction_hash epoch transaction
    | `Consensus_transaction_attested message ->
        transaction_attested state message
end
