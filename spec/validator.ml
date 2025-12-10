open Crypto
open Eth
module GroupId = Id.Make ()
module SignatureId = Id.Make ()
module AddressSet = Set.Make (Address)
module GroupMap = Map.Make (GroupId)
module GroupSet = Set.Make (GroupId)
module IntMap = Map.Make (Int)
module ParticipantMap = Map.Make (FROST.Identifier)
module ParticipantSet = Set.Make (FROST.Identifier)
module StringMap = Map.Make (String)

module ParticipantMTree = MerkleTree.Make (struct
  type t = FROST.Identifier.t * Address.t

  let encode (id, addr) =
    Abi.encode [ `Uint256 (FROST.Identifier.to_int id); `Address addr ]
end)

module NoncesMTree = MerkleTree.Make (struct
  type t = int * FROST.nonces

  let encode (offset, nonces) =
    Abi.encode (`Uint256 offset :: FROST.encode_nonces nonces)
end)

module VerificationMTree = MerkleTree.Make (struct
  type t =
    FROST.(
      Identifier.t * share_commitment * lagrange_coefficient * group_commitment)

  let encode (id, ri, l, r) =
    Abi.encode @@ FROST.encode_verification_share id ri l r
end)

(** A marker type to indicate state that is local to a specific validator and
    only known by them (as opposed to public global shared state). *)
type 'a local = Local of 'a

type group = {
  id : GroupId.t;
  participants : ParticipantSet.t;
  key : FROST.verification_key;
  me : FROST.Identifier.t local;
  share : FROST.signing_share local;
}

type epoch = { epoch : int; group : group }

type rollover_group = {
  id : GroupId.t;
  participants : AddressSet.t;
  key : FROST.verification_key;
  me : FROST.Identifier.t local;
}

type rollover_vss = {
  coefficients : FROST.KeyGen.coefficients local;
  commitments : FROST.KeyGen.commitments ParticipantMap.t;
}

type rollover_complaints = { unresponded : int; total : int }

type rollover_secret_shares = {
  epoch : int;
  group : rollover_group;
  vss : rollover_vss;
  shares : FROST.KeyGen.secret_share option ParticipantMap.t local;
  complaints : rollover_complaints ParticipantMap.t;
  deadline : int;
}

type rollover_finalization = {
  epoch : int;
  group : rollover_group;
  vss : rollover_vss;
  shares : FROST.KeyGen.secret_share ParticipantMap.t local;
  complaints : rollover_complaints ParticipantMap.t;
  confirmations : ParticipantSet.t;
  last_participant : FROST.Identifier.t;
  complaint_deadline : int;
  response_deadline : int;
  confirm_deadline : int;
}

type rollover =
  | Collecting_key_gen_commitments of {
      epoch : int;
      group : GroupId.t;
      participants : AddressSet.t;
      vss : rollover_vss;
      deadline : int;
    }
  | Collecting_key_gen_secret_shares of rollover_secret_shares
  | Confirming_key_gen of rollover_finalization
  | Signing_epoch_rollover of { epoch : epoch; message : string }
  | Staged_epoch of { epoch : epoch }
  | Skipped of { epoch : int }

type preprocess_group = {
  nonces : (FROST.nonces * MerkleTree.proof) IntMap.t;
  pending : (int * FROST.nonces) list option;
}

type sign_status =
  | Waiting_for_sign_request of { responsible : FROST.Identifier.t option }
  | Collecting_sign_nonces of {
      id : SignatureId.t;
      nonces : FROST.nonces_commitments ParticipantMap.t;
    }
  | Collecting_sign_shares of {
      id : SignatureId.t;
      root : string;
      group_commitment : FROST.group_commitment;
      shares : FROST.signature_share ParticipantMap.t;
    }
  | Waiting_for_consensus_attestation of {
      id : SignatureId.t;
      responsible : FROST.Identifier.t option;
    }

type packet =
  | Epoch_rollover of {
      epoch : int;
      rollover_block : int;
      group_key : FROST.verification_key;
    }
  | Transaction_proposal of { epoch : int; hash : string }

type signature = {
  status : sign_status;
  epoch : epoch;
  packet : packet;
  last_participant : FROST.Identifier.t option;
  selection : ParticipantSet.t;
  deadline : int;
}

type state = {
  block : int;
  active_epoch : epoch;
  rollover : rollover;
  preprocess : preprocess_group GroupMap.t local;
  signatures : signature StringMap.t;
}

type transaction = {
  chain_id : int;
  account : Address.t;
  to_ : Address.t;
  value : int;
  operation : [ `Call | `Delegatecall ];
  data : string;
  nonce : int;
}

(** Create a validator parameterized over some configuration. *)
module MakeValidator (Configuration : sig
  val account : Address.t
  val blocks_per_epoch : int
  val all_participants : AddressSet.t
  val consensus : Address.t
  val nonces_chunk_size : int
  val min_remaining_nonces : int
  val key_gen_block_timeout : int
  val signing_block_timeout : int
  val validate_transaction : transaction -> bool
end) =
struct
  let () =
    if Configuration.(min_remaining_nonces > nonces_chunk_size) then
      failwith "invalid configuration"

  let participant_identifier participants address =
    (* We explicitely define the participant identifier to be its index in the
       sorted list of active participants starting at 1. Note that OCaml `Set`
       type is an ordered set, so we don't need to resort here. We implement it
       by spliting the set at the participant's address, so that the cardinal of
       the `left` subset is the number of addresses that are smaller than the
       participant `address`, which allows us to trivially compute the FROST
       identifier for that participant address. Note that we return `None` in
       case the `address` is not present in the `participants` set. *)
    let left, present, _ = AddressSet.split address participants in
    if present then
      Some (FROST.Identifier.of_int (1 + AddressSet.cardinal left))
    else None

  let participant_address participants identifier =
    let i = FROST.Identifier.to_int identifier - 1 in
    List.nth (AddressSet.to_list participants) i

  let participants_tree participants =
    (* Note that sets are ordered in OCaml, so no need to sort the address set
       before computing each participant's identifier. *)
    AddressSet.to_list participants
    |> List.mapi (fun i a -> (FROST.Identifier.of_int (i + 1), a))

  let participant_identifiers participants =
    participants_tree participants |> List.map (fun (id, _) -> id)

  let participant_set participants =
    ParticipantSet.of_list @@ participant_identifiers participants

  let remove_missing_participants participants items =
    let missing =
      participants_tree participants
      |> ParticipantMap.of_list
      |> ParticipantMap.fold
           (fun id _ missing -> ParticipantMap.remove id missing)
           items
    in
    ParticipantMap.fold
      (fun _ address remaining -> AddressSet.remove address remaining)
      missing participants

  let participanting_selection items =
    ParticipantMap.fold
      (fun id _ set -> ParticipantSet.add id set)
      items ParticipantSet.empty

  let group_threshold count =
    (* For our consensus to be resilient to intermittent failures, we have a
       define the following parameters:
       - The minimum number of participants in a group must be strictly greater
         than 2/3rds of the set of all validators.
       - The threshold for a group is strictly greater than 1/2

       This implies a Byzantine fault tolerance of 33%, and is chosen to be
       this way to not make it possible to ever roll into a group without an
       absolute majority of honest participants. *)
    let ceil_div q d = (q + d - 1) / d in
    let ( /^ ) = ceil_div in
    (* For our consensus to never get stuck due to dishonest participants, we
       require more than 2/3rds of the total participant set to participate. Any
       less and dishonest participants can be a majority in any group if you
       consider intermittent failures from honest participants. *)
    let min_count =
      ((2 * AddressSet.cardinal Configuration.all_participants) + 1) /^ 3
    in
    if count >= min_count then
      (* The threshold is always the absolute majority of participants. *)
      Some ((count / 2) + 1)
    else None

  let compute_group_id participants count threshold context =
    let participants_root =
      ParticipantMTree.root @@ participants_tree participants
    in
    Abi.encode
      [
        `Bytes32 participants_root;
        `Uint64 count;
        `Uint64 threshold;
        `Bytes32 context;
      ]
    |> Keccak256.hash |> GroupId.of_string

  let key_gen_context epoch =
    let version = 0 in
    Abi.encode_packed
      [ `Uint32 version; `Address Configuration.consensus; `Uint64 epoch ]

  let key_gen_and_commit state participants =
    let epoch = 1 + (state.block / Configuration.blocks_per_epoch) in
    let identifier =
      participant_identifier participants Configuration.account
    in
    let count = AddressSet.cardinal participants in
    match (identifier, group_threshold count) with
    | Some identifier, Some threshold ->
        let context = key_gen_context epoch in
        let coefficients = FROST.KeyGen.random_coefficients threshold in
        let state' =
          {
            state with
            rollover =
              Collecting_key_gen_commitments
                {
                  epoch;
                  group = compute_group_id participants count threshold context;
                  participants;
                  vss =
                    {
                      commitments = ParticipantMap.empty;
                      coefficients = Local coefficients;
                    };
                  deadline = state.block + Configuration.key_gen_block_timeout;
                };
          }
        in
        let participants' = participants_tree participants in
        let actions =
          [
            `Coordinator_key_gen_and_commit
              ( ParticipantMTree.root participants',
                count,
                threshold,
                context,
                identifier,
                ParticipantMTree.proof
                  (identifier, Configuration.account)
                  participants',
                FROST.KeyGen.commit coefficients );
          ]
        in
        (state', actions)
    | _ ->
        (* In case the epoch rollover is continuing without me, or if the number
           of participants becomes too small, then we skip the rollover until
           the next epoch, in the hopes that we and/or other validators recover
           enough to continue with consensus. *)
        let state' = { state with rollover = Skipped { epoch } } in
        (state', [])

  let rollover_block epoch = epoch * Configuration.blocks_per_epoch

  let epoch_rollover state =
    let key_gen_and_commit_with_all st =
      (* Always use the full participant set at the start of the epoch, even
         if they were previously misbahaving. In the future, the participant
         selection can be stricter and inspect onchain state of the staking
         contract. *)
      key_gen_and_commit st Configuration.all_participants
    in
    match state.rollover with
    | Collecting_key_gen_commitments { epoch; _ }
    | Collecting_key_gen_secret_shares { epoch; _ }
    | Confirming_key_gen { epoch; _ }
    | Signing_epoch_rollover { epoch = { epoch; _ }; _ }
    | Skipped { epoch } ->
        if rollover_block epoch = state.block then
          (* The previous KeyGen ceremony was skipped or took too, try again. *)
          key_gen_and_commit_with_all state
        else (state, [])
    | Staged_epoch { epoch = staged } ->
        if rollover_block staged.epoch = state.block then
          let state' = { state with active_epoch = staged } in
          key_gen_and_commit_with_all state'
        else (state, [])

  let key_gen_wait state =
    match state.rollover with
    | Collecting_key_gen_commitments { participants; deadline; vss; _ } ->
        if deadline = state.block then
          let participants' =
            remove_missing_participants participants vss.commitments
          in
          key_gen_and_commit state participants'
        else (state, [])
    | Collecting_key_gen_secret_shares
        { group; deadline; shares = Local shares; _ } ->
        if deadline = state.block then
          let participants' =
            remove_missing_participants group.participants shares
          in
          key_gen_and_commit state participants'
        else (state, [])
    | Confirming_key_gen
        {
          group;
          complaints;
          confirmations;
          response_deadline;
          confirm_deadline;
          _;
        } ->
        if response_deadline = state.block then
          (* Remove participants that did not respond to complaints in time. *)
          let participants' =
            ParticipantMap.fold
              (fun id { unresponded; _ } participants ->
                if unresponded <> 0 then
                  let address = participant_address participants id in
                  AddressSet.remove address participants
                else participants)
              complaints group.participants
          in
          key_gen_and_commit state participants'
        else if confirm_deadline = state.block then
          (* Remove participants that did not confirm in time. *)
          let participants' =
            ParticipantSet.fold
              (fun id participants ->
                let address = participant_address participants id in
                AddressSet.remove address participants)
              confirmations group.participants
          in
          key_gen_and_commit state participants'
        else (state, [])
    (* Other KeyGen states either don't have deadlines, or they are handled in
       the signing flow. *)
    | Signing_epoch_rollover _ | Skipped _ | Staged_epoch _ -> (state, [])

  let key_gen_commitment state group_id identifier commitment =
    match state.rollover with
    | Collecting_key_gen_commitments
        { epoch; group; participants; vss; deadline }
      when GroupId.equal group group_id ->
        if FROST.KeyGen.verify_proof commitment then
          let commitments' =
            ParticipantMap.add identifier commitment vss.commitments
          in
          if
            AddressSet.cardinal participants
            = ParticipantMap.cardinal commitments'
          then
            let me =
              participant_identifier participants Configuration.account
              |> Option.get
            in
            let (Local coefficients) = vss.coefficients in
            let shares =
              ParticipantMap.singleton me
                (Some (FROST.KeyGen.secret_share coefficients me))
            in
            let encrypted_shares =
              participant_set participants
              |> ParticipantSet.remove me |> ParticipantSet.to_list
              |> List.map (fun id ->
                  let share = FROST.KeyGen.secret_share coefficients id in
                  let participant_commitments =
                    ParticipantMap.find id commitments'
                  in
                  FROST.KeyGen.encrypt_secret_share coefficients
                    participant_commitments share)
            in
            let key =
              FROST.KeyGen.group_public_key
                (ParticipantMap.to_list commitments')
            in
            let rollover' =
              Collecting_key_gen_secret_shares
                {
                  epoch;
                  group = { id = group; participants; key; me = Local me };
                  vss;
                  shares = Local shares;
                  complaints = ParticipantMap.empty;
                  deadline = state.block + Configuration.key_gen_block_timeout;
                }
            in
            ( { state with rollover = rollover' },
              [
                `Coordinator_key_gen_secret_share_with_callback
                  ( group,
                    encrypted_shares,
                    ( Configuration.consensus,
                      Abi.encode
                        [ `Uint64 epoch; `Uint64 (rollover_block epoch) ] ) );
              ] )
          else
            let rollover' =
              Collecting_key_gen_commitments
                {
                  epoch;
                  group;
                  vss = { vss with commitments = commitments' };
                  participants;
                  deadline;
                }
            in
            ({ state with rollover = rollover' }, [])
        else
          let address = participant_address participants identifier in
          let participants' = AddressSet.remove address participants in
          key_gen_and_commit state participants'
    | _ -> (state, [])

  let key_gen_secret_shares state group_id identifier secret_shares =
    match state.rollover with
    | Collecting_key_gen_secret_shares st
      when GroupId.equal st.group.id group_id -> begin
        let (Local me) = st.group.me in
        let (Local coefficients) = st.vss.coefficients in
        let secret_share, actions_complaint =
          let i =
            FROST.Identifier.to_int me
            - if FROST.Identifier.compare me identifier < 0 then 1 else 2
          in
          let encrypted = List.nth secret_shares i in
          let participant_commitments =
            ParticipantMap.find identifier st.vss.commitments
          in
          let value =
            FROST.KeyGen.decrypt_secret_share coefficients
              participant_commitments encrypted
          in
          if FROST.KeyGen.verify_secret_share participant_commitments value then
            (Some value, [])
          else (None, [ `Coordinator_key_gen_complain (group_id, identifier) ])
        in
        let (Local shares) = st.shares in
        let shares' = ParticipantMap.add identifier secret_share shares in
        let state', actions_confirm =
          if
            AddressSet.cardinal st.group.participants
            = ParticipantMap.cardinal shares'
          then
            let valid_shares =
              ParticipantMap.filter_map (fun _ -> Fun.id) shares'
            in
            let rollover' =
              Confirming_key_gen
                {
                  epoch = st.epoch;
                  group = st.group;
                  vss = st.vss;
                  shares = Local valid_shares;
                  confirmations = ParticipantSet.empty;
                  complaints = st.complaints;
                  last_participant = identifier;
                  (* Right now, we are overly generous with the deadlines here.
                     In the future we can tighten this up by dynamically setting
                     the response and confirm deadlines based on the latest
                     complaint and response block respectively. Since KeyGen
                     happens seldomly, and does not need to be quick. *)
                  complaint_deadline =
                    state.block + Configuration.key_gen_block_timeout;
                  response_deadline =
                    state.block + (2 * Configuration.key_gen_block_timeout);
                  confirm_deadline =
                    state.block + (3 * Configuration.key_gen_block_timeout);
                }
            in
            let state' = { state with rollover = rollover' } in
            let actions =
              if
                ParticipantMap.cardinal valid_shares
                = ParticipantMap.cardinal shares'
              then
                [
                  `Coordinator_key_gen_confirm_with_callback
                    ( st.group.id,
                      ( Configuration.consensus,
                        Abi.encode
                          [
                            `Uint64 st.epoch; `Uint64 (rollover_block st.epoch);
                          ] ) );
                ]
              else []
            in
            (state', actions)
          else
            let rollover' =
              Collecting_key_gen_secret_shares
                { st with shares = Local shares' }
            in
            let state' = { state with rollover = rollover' } in
            (state', [])
        in
        (state', actions_complaint @ actions_confirm)
      end
    | _ -> (state, [])

  let key_gen_confirm state group_id identifier =
    match state.rollover with
    | Confirming_key_gen st when GroupId.equal st.group.id group_id ->
        let confirmations' = ParticipantSet.add identifier st.confirmations in
        if
          AddressSet.cardinal st.group.participants
          = ParticipantSet.cardinal confirmations'
        then
          let packet =
            Epoch_rollover
              {
                epoch = st.epoch;
                rollover_block = rollover_block st.epoch;
                group_key = st.group.key;
              }
          in
          let message = Abi.hash_typed_data packet in
          (* At this point, we should always have the correct number of shares,
             that is, unless someone was able to confirm on our behalf. *)
          let (Local shares) = st.shares in
          let share =
            FROST.KeyGen.signing_share @@ ParticipantMap.to_list shares
          in
          let epoch =
            {
              epoch = st.epoch;
              group =
                {
                  id = st.group.id;
                  participants = participant_set st.group.participants;
                  key = st.group.key;
                  me = st.group.me;
                  share = Local share;
                };
            }
          in
          let rollover' = Signing_epoch_rollover { epoch; message } in
          let signatures' =
            StringMap.add message
              {
                status =
                  Waiting_for_sign_request { responsible = Some identifier };
                epoch = state.active_epoch;
                packet;
                last_participant = Some st.last_participant;
                selection = state.active_epoch.group.participants;
                deadline = state.block + Configuration.signing_block_timeout;
              }
              state.signatures
          in
          let state' =
            { state with rollover = rollover'; signatures = signatures' }
          in
          (state', [])
        else
          let rollover' =
            Confirming_key_gen { st with confirmations = confirmations' }
          in
          let state' = { state with rollover = rollover' } in
          (state', [])
    | _ -> (state, [])

  let key_gen_complaint state group_id plaintiff accused =
    let add_complaint accused complaints =
      ParticipantMap.update accused
        (function
          | Some { unresponded; total } ->
              Some { unresponded = unresponded + 1; total = total + 1 }
          | None -> Some { unresponded = 1; total = 1 })
        complaints
    in
    let respond_complaint { id; me = Local me; _ }
        { coefficients = Local coefficients; _ } =
      if FROST.Identifier.equal me accused then
        let secret_share = FROST.KeyGen.secret_share coefficients plaintiff in
        [ `Coordinator_key_gen_complaint_response (id, me, secret_share) ]
      else []
    in
    let rollover', actions =
      match state.rollover with
      | Collecting_key_gen_secret_shares st
        when GroupId.equal st.group.id group_id ->
          let complaints' = add_complaint accused st.complaints in
          ( Collecting_key_gen_secret_shares { st with complaints = complaints' },
            respond_complaint st.group st.vss )
      | Confirming_key_gen st when GroupId.equal st.group.id group_id ->
          (* Note that we drop complaints that are received past the deadline. *)
          if state.block <= st.complaint_deadline then
            let complaints' = add_complaint accused st.complaints in
            ( Confirming_key_gen { st with complaints = complaints' },
              respond_complaint st.group st.vss )
          else (state.rollover, [])
      | _ -> (state.rollover, [])
    in
    let state' = { state with rollover = rollover' } in
    (state', actions)

  let key_gen_complaint_response state group_id plaintiff accused secret_share =
    let record_response accused complaints =
      let c = ParticipantMap.find accused complaints in
      ParticipantMap.add accused
        { c with unresponded = c.unresponded - 1 }
        complaints
    in
    match state.rollover with
    | Collecting_key_gen_secret_shares st
      when GroupId.equal st.group.id group_id ->
        let accused_commitments =
          ParticipantMap.find accused st.vss.commitments
        in
        if FROST.KeyGen.verify_secret_share accused_commitments secret_share
        then
          let (Local me) = st.group.me in
          let (Local shares) = st.shares in
          let shares' =
            if FROST.Identifier.equal me plaintiff then
              ParticipantMap.add accused (Some secret_share) shares
            else shares
          in
          let complaints' = record_response accused st.complaints in
          let rollover' =
            Collecting_key_gen_secret_shares
              { st with shares = Local shares'; complaints = complaints' }
          in
          let state' = { state with rollover = rollover' } in
          (state', [])
        else
          let address = participant_address st.group.participants accused in
          let participants' = AddressSet.remove address st.group.participants in
          key_gen_and_commit state participants'
    | Confirming_key_gen st when GroupId.equal st.group.id group_id ->
        (* Note that we drop complaint responses that come too late. This should
           never really happen in practice (since honest validators ignore
           complaints that come in too late), but can still happen under certain
           re-org conditions. *)
        if state.block <= st.response_deadline then
          let accused_commitments =
            ParticipantMap.find accused st.vss.commitments
          in
          if FROST.KeyGen.verify_secret_share accused_commitments secret_share
          then
            let (Local me) = st.group.me in
            let (Local shares) = st.shares in
            let shares', actions =
              if FROST.Identifier.equal me plaintiff then
                let shares' = ParticipantMap.add accused secret_share shares in
                let actions =
                  if
                    AddressSet.cardinal st.group.participants
                    = ParticipantMap.cardinal shares'
                  then
                    [
                      `Coordinator_key_gen_confirm_with_callback
                        ( st.group.id,
                          ( Configuration.consensus,
                            Abi.encode
                              [
                                `Uint64 st.epoch;
                                `Uint64 (rollover_block st.epoch);
                              ] ) );
                    ]
                  else []
                in
                (shares', actions)
              else (shares, [])
            in
            let complaints' = record_response accused st.complaints in
            let rollover' =
              Confirming_key_gen
                { st with shares = Local shares'; complaints = complaints' }
            in
            let state' = { state with rollover = rollover' } in
            (state', actions)
          else
            let address = participant_address st.group.participants accused in
            let participants' =
              AddressSet.remove address st.group.participants
            in
            key_gen_and_commit state participants'
        else (state, [])
    | _ -> (state, [])

  let preprocess state group_id chunk nonces_commitments =
    let (Local preprocess) = state.preprocess in
    let state' =
      match GroupMap.find_opt group_id preprocess with
      | Some { nonces; pending = Some pending }
        when nonces_commitments = NoncesMTree.root pending ->
          let starting_sequence = chunk * Configuration.nonces_chunk_size in
          let nonces' =
            List.fold_left
              (fun nonces' (offset, n) ->
                let sequence = starting_sequence + offset in
                let proof = NoncesMTree.proof (offset, n) pending in
                IntMap.add sequence (n, proof) nonces')
              nonces pending
          in
          let preprocess' =
            GroupMap.add group_id
              { nonces = nonces'; pending = None }
              preprocess
          in
          { state with preprocess = Local preprocess' }
      | _ -> state
    in
    (state', [])

  let signing_ceremony_nonce state group sequence =
    let (Local preprocess) = state.preprocess in
    let (Local share) = group.share in
    match GroupMap.find_opt group.id preprocess with
    | Some { nonces; pending } ->
        let _, entry, nonces' = IntMap.split sequence nonces in
        let pending', actions =
          if
            IntMap.cardinal nonces' < Configuration.min_remaining_nonces
            && Option.is_none pending
          then
            let nonces_chunk =
              List.init Configuration.nonces_chunk_size (fun offset ->
                  (offset, FROST.generate_nonces share))
            in
            let nonces_commitment = NoncesMTree.root nonces_chunk in
            let pending' = Some nonces_chunk in
            (pending', [ `Coordinator_preprocess (group.id, nonces_commitment) ])
          else (pending, [])
        in
        let preprocess' =
          GroupMap.add group.id
            { nonces = nonces'; pending = pending' }
            preprocess
        in
        let state' = { state with preprocess = Local preprocess' } in
        (entry, state', actions)
    | None -> (None, state, [])

  let signing_ceremony_request state group_id message signature_id sequence =
    match StringMap.find_opt message state.signatures with
    | Some signature when GroupId.equal signature.epoch.group.id group_id ->
      begin
        match signature.status with
        | Waiting_for_sign_request _ -> begin
            let signature' =
              {
                signature with
                status =
                  Collecting_sign_nonces
                    { id = signature_id; nonces = ParticipantMap.empty };
                deadline = state.block + Configuration.signing_block_timeout;
              }
            in
            let signatures' =
              StringMap.add message signature' state.signatures
            in
            let state' = { state with signatures = signatures' } in
            let nonces, state', actions =
              signing_ceremony_nonce state' signature.epoch.group sequence
            in
            match nonces with
            | Some (nonces, proof) ->
                ( state',
                  `Coordinator_sign_reveal_nonces (nonces, proof) :: actions )
            | None -> (state', actions)
          end
        | _ -> (state, [])
      end
    | _ -> (state, [])

  let sign_share_with_callback signature message signature_id nonces =
    let { key; me = Local me; share = Local share; _ } =
      signature.epoch.group
    in
    let signature_share, group_commitment, verification_shares =
      FROST.signature_share me share message key (ParticipantMap.to_list nonces)
    in
    let _, share_commitment, lagrange_coefficient =
      List.find
        (fun (id, _, _) -> FROST.Identifier.equal me id)
        verification_shares
    in
    let verification_tree =
      List.map
        (fun (id, ri, l) -> (id, ri, l, group_commitment))
        verification_shares
    in
    let root = VerificationMTree.root verification_tree in
    let state' =
      Collecting_sign_shares
        {
          id = signature_id;
          root;
          group_commitment;
          shares = ParticipantMap.empty;
        }
    in
    let proof =
      VerificationMTree.proof
        (me, share_commitment, lagrange_coefficient, group_commitment)
    in
    let callback_context =
      match signature.packet with
      | Epoch_rollover { epoch; rollover_block; _ } ->
          Abi.encode_call "proposeEpoch"
            [ `Uint64 epoch; `Uint64 rollover_block ]
      | Transaction_proposal { epoch; hash } ->
          Abi.encode_call "attestTransaction" [ `Uint64 epoch; `Bytes32 hash ]
    in
    ( state',
      `Coordinator_sign_share_with_callback
        ( signature_id,
          (share_commitment, root),
          signature_share,
          proof,
          (Configuration.consensus, callback_context) ) )

  let signing_ceremony_reveal_nonce state signature_id identifier
      nonces_commitments =
    match
      List.find_map (fun (message, signature) ->
          match signature.status with
          | Collecting_sign_nonces { id; nonces }
            when SignatureId.equal id signature_id ->
              Some (message, signature, nonces)
          | _ -> None)
      @@ StringMap.to_list state.signatures
    with
    | Some (message, signature, nonces) ->
        let nonces' = ParticipantMap.add identifier nonces_commitments nonces in
        let state', actions =
          if
            ParticipantMap.cardinal nonces'
            = ParticipantSet.cardinal signature.selection
          then
            let state', action =
              sign_share_with_callback signature message signature_id nonces'
            in
            (state', [ action ])
          else
            let state' =
              Collecting_sign_nonces { id = signature_id; nonces = nonces' }
            in
            (state', [])
        in
        let signature' =
          {
            signature with
            status = state';
            deadline = state.block + Configuration.signing_block_timeout;
          }
        in
        let signatures' = StringMap.add message signature' state.signatures in
        let state' = { state with signatures = signatures' } in
        (state', actions)
    | None -> (state, [])

  let signing_ceremony_share state signature_id identifier signature_share
      binding_root =
    let state' =
      match
        List.find_map (fun (message, signature) ->
            match signature.status with
            | Collecting_sign_shares { id; root; group_commitment; shares }
              when SignatureId.equal id signature_id
                   && String.equal root binding_root ->
                Some (message, signature, group_commitment, shares)
            | _ -> None)
        @@ StringMap.to_list state.signatures
      with
      | Some (message, signature, group_commitment, shares) ->
          let shares' = ParticipantMap.add identifier signature_share shares in
          let state' =
            if
              ParticipantMap.cardinal shares'
              = ParticipantSet.cardinal signature.selection
            then
              let state' =
                Waiting_for_consensus_attestation
                  { id = signature_id; responsible = Some identifier }
              in
              state'
            else
              Collecting_sign_shares
                {
                  id = signature_id;
                  root = binding_root;
                  group_commitment;
                  shares = shares';
                }
          in
          let signature' =
            {
              signature with
              status = state';
              deadline = state.block + Configuration.signing_block_timeout;
            }
          in
          let signatures' = StringMap.add message signature' state.signatures in
          let state' = { state with signatures = signatures' } in
          state'
      | None -> state
    in
    (state', [])

  let signatures_wait state =
    let signatures', actions =
      StringMap.fold
        (fun message signature (signatures, actions) ->
          let signature', actions =
            if signature.deadline = state.block then begin
              let selection' =
                match signature.status with
                | Collecting_sign_nonces { nonces; _ } ->
                    participanting_selection nonces
                | Collecting_sign_shares { shares; _ } ->
                    participanting_selection shares
                | Waiting_for_sign_request { responsible }
                | Waiting_for_consensus_attestation { responsible; _ } -> (
                    match responsible with
                    | Some responsible ->
                        ParticipantSet.remove responsible signature.selection
                    | None -> ParticipantSet.empty)
              in
              let threshold =
                group_threshold
                @@ ParticipantSet.cardinal signature.epoch.group.participants
                |> Option.get
              in
              let insufficient_signers =
                ParticipantSet.cardinal selection' < threshold
              in
              if insufficient_signers then
                (* If we don't have enough signers for a ceremony, we need to
                   drop the signature request. *)
                (None, [])
              else
                let state', action, actor =
                  match signature.status with
                  | Waiting_for_consensus_attestation { id = signature_id; _ }
                    ->
                      (* As an optimization, we just re-attest if we already have
                         a valid signature but the responsible participant did not
                         do it in time. The punishment for doing this is foregoing
                         rewards for that attestation. *)
                      let state' =
                        Waiting_for_consensus_attestation
                          {
                            id = signature_id;
                            responsible = signature.last_participant;
                          }
                      in
                      (* The actual attestation action depends on the packet we are
                         attesting to. *)
                      let action =
                        match signature.packet with
                        | Epoch_rollover { epoch; rollover_block; group_key } ->
                            `Consensus_stage_epoch
                              (epoch, rollover_block, group_key, signature_id)
                        | Transaction_proposal { epoch; hash } ->
                            `Consensus_attest_transaction
                              (epoch, hash, signature_id)
                      in
                      (state', action, signature.last_participant)
                  | Collecting_sign_nonces { id; nonces; _ } ->
                      (* Continue the signing process with only the participants
                         which provided nonces. Note that this is sound given the
                         asserted assumption above - i.e. we always have a
                         threshold of well-behaving signers. *)
                      let state', action =
                        sign_share_with_callback signature message id nonces
                      in
                      (state', action, None)
                  | _ ->
                      ( Waiting_for_sign_request
                          { responsible = signature.last_participant },
                        `Coordinator_sign (signature.epoch.group.id, message),
                        signature.last_participant )
                in
                let signature' =
                  {
                    signature with
                    status = state';
                    last_participant = None;
                    selection = selection';
                    deadline = state.block + Configuration.signing_block_timeout;
                  }
                in
                let (Local me) = signature.epoch.group.me in
                let actions' =
                  match actor with
                  | None -> action :: actions
                  | Some address when FROST.Identifier.equal address me ->
                      action :: actions
                  | _ -> actions
                in
                (Some signature', actions')
            end
            else (Some signature, actions)
          in
          let signatures' =
            match signature' with
            | Some signature' -> StringMap.add message signature' signatures
            | None -> StringMap.remove message signatures
          in
          (signatures', actions))
        state.signatures (StringMap.empty, [])
    in
    let state' = { state with signatures = signatures' } in
    (state', actions)

  let epoch_staged state proposed_epoch =
    match state.rollover with
    | Signing_epoch_rollover { epoch; message }
      when epoch.epoch = proposed_epoch ->
        let rollover' = Staged_epoch { epoch } in
        let (Local share) = epoch.group.share in
        let nonces_chunk =
          List.init Configuration.nonces_chunk_size (fun offset ->
              (offset, FROST.generate_nonces share))
        in
        let nonces_commitment = NoncesMTree.root nonces_chunk in
        let (Local preprocess) = state.preprocess in
        let preprocess' =
          GroupMap.add epoch.group.id
            { nonces = IntMap.empty; pending = Some nonces_chunk }
            preprocess
        in
        let signatures' = StringMap.remove message state.signatures in
        let state' =
          {
            state with
            rollover = rollover';
            preprocess = Local preprocess';
            signatures = signatures';
          }
        in
        (state', [ `Coordinator_preprocess (epoch.group.id, nonces_commitment) ])
    | _ -> (state, [])

  let transaction_proposed state message transaction_hash epoch transaction =
    if
      epoch = state.active_epoch.epoch
      && Configuration.validate_transaction transaction
    then
      let signatures' =
        StringMap.add message
          {
            status = Waiting_for_sign_request { responsible = None };
            epoch = state.active_epoch;
            packet = Transaction_proposal { epoch; hash = transaction_hash };
            last_participant = None;
            selection = state.active_epoch.group.participants;
            deadline = state.block + Configuration.signing_block_timeout;
          }
          state.signatures
      in
      let state' = { state with signatures = signatures' } in
      (state', [])
    else (state, [])

  let transaction_attested state message =
    let signatures' = StringMap.remove message state.signatures in
    ({ state with signatures = signatures' }, [])

  let garbage_collect state =
    let groups = GroupSet.singleton state.active_epoch.group.id in
    let groups =
      match state.rollover with
      | Staged_epoch { epoch } -> GroupSet.add epoch.group.id groups
      | _ -> groups
    in
    let groups =
      StringMap.fold
        (fun _ signature groups -> GroupSet.add signature.epoch.group.id groups)
        state.signatures groups
    in
    let (Local preprocess) = state.preprocess in
    let preprocess' =
      GroupSet.fold
        (fun group preprocess' ->
          match GroupMap.find_opt group preprocess with
          | Some p -> GroupMap.add group p preprocess'
          | None -> preprocess')
        groups GroupMap.empty
    in
    { state with preprocess = Local preprocess' }

  (** The state transition function for a validator. *)
  let transition state event =
    let state', actions =
      match event with
      | `Chain_block block_number ->
          let state' = { state with block = block_number } in
          let state', actions_r = epoch_rollover state' in
          let state', actions_kgw = key_gen_wait state' in
          let state', actions_sw = signatures_wait state' in
          (state', actions_r @ actions_kgw @ actions_sw)
      | `Coordinator_key_gen_committed (group_id, identifier, commitment, _) ->
          key_gen_commitment state group_id identifier commitment
      | `Coordinator_key_gen_secret_shared
          (group_id, identifier, secret_shares, _) ->
          key_gen_secret_shares state group_id identifier secret_shares
      | `Coordinator_key_gen_confirm (group_id, identifier) ->
          key_gen_confirm state group_id identifier
      | `Coordinator_key_gen_complained (group_id, plaintiff, accused) ->
          key_gen_complaint state group_id plaintiff accused
      | `Coordinator_key_gen_complaint_response
          (group_id, plaintiff, accused, secret_share) ->
          key_gen_complaint_response state group_id plaintiff accused
            secret_share
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
    in
    (garbage_collect state', actions)
end
